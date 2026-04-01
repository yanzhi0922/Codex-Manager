use bytes::Bytes;
use codexmanager_core::storage::{now_ts, AggregateApi, Storage};
use reqwest::header::{HeaderName, HeaderValue};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tiny_http::Request;

use crate::aggregate_api::{
    build_aggregate_api_unversioned_fallback_url, build_aggregate_api_upstream_url,
    AGGREGATE_API_PROVIDER_CLAUDE, AGGREGATE_API_PROVIDER_CODEX,
};
use crate::gateway::request_log::RequestLogUsage;

const AGGREGATE_API_RETRY_ATTEMPTS_PER_CHANNEL: usize = 3;
const DEFAULT_AGGREGATE_API_HEALTH_SCORE: i32 = 100;
const MIN_AGGREGATE_API_HEALTH_SCORE: i32 = 0;
const MAX_AGGREGATE_API_HEALTH_SCORE: i32 = 200;
const AGGREGATE_API_HEALTH_TTL_SECS: i64 = 24 * 60 * 60;
const AGGREGATE_API_INFLIGHT_PENALTY: i32 = 250;
const AGGREGATE_API_COOLDOWN_PENALTY: i32 = 120;

#[derive(Debug, Clone, Default)]
struct AggregateApiQualityRecord {
    health_score: i32,
    updated_at: i64,
}

#[derive(Default)]
struct AggregateApiRuntimeState {
    next_start_by_provider: HashMap<String, usize>,
    inflight_by_api_id: HashMap<String, usize>,
    quality_by_api_id: HashMap<String, AggregateApiQualityRecord>,
}

static AGGREGATE_API_RUNTIME_STATE: OnceLock<Mutex<AggregateApiRuntimeState>> = OnceLock::new();

struct AggregateApiInflightGuard {
    api_id: String,
}

impl Drop for AggregateApiInflightGuard {
    fn drop(&mut self) {
        let lock = AGGREGATE_API_RUNTIME_STATE
            .get_or_init(|| Mutex::new(AggregateApiRuntimeState::default()));
        let mut state = crate::lock_utils::lock_recover(lock, "aggregate_api_runtime_state");
        if let Some(value) = state.inflight_by_api_id.get_mut(self.api_id.as_str()) {
            if *value > 1 {
                *value -= 1;
            } else {
                state.inflight_by_api_id.remove(self.api_id.as_str());
            }
        }
    }
}

fn should_skip_forward_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization"
            | "x-api-key"
            | "api-key"
            | "content-length"
            | "connection"
            | "proxy-authorization"
            | "proxy-authenticate"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "host"
    )
}

fn respond_error(request: Request, status: u16, message: &str, trace_id: Option<&str>) {
    let response = super::super::super::error_response::terminal_text_response(
        status,
        message.to_string(),
        trace_id,
    );
    let _ = request.respond(response);
}

fn normalize_candidate_order(mut candidates: Vec<AggregateApi>) -> Vec<AggregateApi> {
    candidates.sort_by(|left, right| {
        left.sort
            .cmp(&right.sort)
            .then(right.created_at.cmp(&left.created_at))
            .then(left.id.cmp(&right.id))
    });
    candidates
}

fn acquire_aggregate_api_inflight(api_id: &str) -> AggregateApiInflightGuard {
    let lock =
        AGGREGATE_API_RUNTIME_STATE.get_or_init(|| Mutex::new(AggregateApiRuntimeState::default()));
    let mut state = crate::lock_utils::lock_recover(lock, "aggregate_api_runtime_state");
    let entry = state
        .inflight_by_api_id
        .entry(api_id.to_string())
        .or_insert(0);
    *entry += 1;
    AggregateApiInflightGuard {
        api_id: api_id.to_string(),
    }
}

fn aggregate_api_inflight_count(api_id: &str) -> usize {
    let lock =
        AGGREGATE_API_RUNTIME_STATE.get_or_init(|| Mutex::new(AggregateApiRuntimeState::default()));
    let state = crate::lock_utils::lock_recover(lock, "aggregate_api_runtime_state");
    state.inflight_by_api_id.get(api_id).copied().unwrap_or(0)
}

fn aggregate_api_health_delta(status_code: u16) -> i32 {
    match status_code {
        200..=299 => 4,
        429 => -15,
        500..=599 => -10,
        401 | 403 => -18,
        400..=499 => -8,
        _ => -2,
    }
}

fn aggregate_api_quality_expired(record: &AggregateApiQualityRecord, now: i64) -> bool {
    record.updated_at + AGGREGATE_API_HEALTH_TTL_SECS <= now
}

fn aggregate_api_health_score(api_id: &str) -> i32 {
    let lock =
        AGGREGATE_API_RUNTIME_STATE.get_or_init(|| Mutex::new(AggregateApiRuntimeState::default()));
    let mut state = crate::lock_utils::lock_recover(lock, "aggregate_api_runtime_state");
    let now = now_ts();
    let Some(record) = state.quality_by_api_id.get(api_id).cloned() else {
        return DEFAULT_AGGREGATE_API_HEALTH_SCORE;
    };
    if aggregate_api_quality_expired(&record, now) {
        state.quality_by_api_id.remove(api_id);
        return DEFAULT_AGGREGATE_API_HEALTH_SCORE;
    }
    record.health_score.clamp(
        MIN_AGGREGATE_API_HEALTH_SCORE,
        MAX_AGGREGATE_API_HEALTH_SCORE,
    )
}

fn record_aggregate_api_quality(api_id: &str, status_code: u16) {
    let lock =
        AGGREGATE_API_RUNTIME_STATE.get_or_init(|| Mutex::new(AggregateApiRuntimeState::default()));
    let mut state = crate::lock_utils::lock_recover(lock, "aggregate_api_runtime_state");
    let now = now_ts();
    let record = state
        .quality_by_api_id
        .entry(api_id.to_string())
        .or_default();
    if record.updated_at == 0 || aggregate_api_quality_expired(record, now) {
        record.health_score = DEFAULT_AGGREGATE_API_HEALTH_SCORE;
    }
    record.updated_at = now;
    record.health_score = (record.health_score + aggregate_api_health_delta(status_code)).clamp(
        MIN_AGGREGATE_API_HEALTH_SCORE,
        MAX_AGGREGATE_API_HEALTH_SCORE,
    );
}

fn aggregate_api_last_test_bonus(last_test_status: Option<&str>) -> i32 {
    match last_test_status
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("success") => 80,
        Some("failed") => -AGGREGATE_API_COOLDOWN_PENALTY,
        _ => 0,
    }
}

fn aggregate_api_runtime_score(candidate: &AggregateApi) -> i32 {
    aggregate_api_health_score(candidate.id.as_str()) * 4
        + aggregate_api_last_test_bonus(candidate.last_test_status.as_deref())
        - aggregate_api_inflight_count(candidate.id.as_str()) as i32
            * AGGREGATE_API_INFLIGHT_PENALTY
}

fn next_aggregate_api_start_index(provider_type: &str, candidate_count: usize) -> usize {
    let lock =
        AGGREGATE_API_RUNTIME_STATE.get_or_init(|| Mutex::new(AggregateApiRuntimeState::default()));
    let mut state = crate::lock_utils::lock_recover(lock, "aggregate_api_runtime_state");
    let entry = state
        .next_start_by_provider
        .entry(provider_type.to_string())
        .or_insert(0);
    let start = *entry % candidate_count;
    *entry = (start + 1) % candidate_count;
    start
}

fn smart_balance_aggregate_api_candidates(candidates: &mut [AggregateApi], provider_type: &str) {
    if candidates.len() <= 1 {
        return;
    }

    let start = next_aggregate_api_start_index(provider_type, candidates.len());
    if start > 0 {
        candidates.rotate_left(start);
    }

    let Some((best_idx, _)) =
        candidates
            .iter()
            .enumerate()
            .max_by(|(left_idx, left), (right_idx, right)| {
                aggregate_api_runtime_score(left)
                    .cmp(&aggregate_api_runtime_score(right))
                    .then_with(|| right_idx.cmp(left_idx))
            })
    else {
        return;
    };
    if best_idx > 0 {
        candidates.swap(0, best_idx);
    }
}

pub(super) fn clear_runtime_state() {
    let lock =
        AGGREGATE_API_RUNTIME_STATE.get_or_init(|| Mutex::new(AggregateApiRuntimeState::default()));
    let mut state = crate::lock_utils::lock_recover(lock, "aggregate_api_runtime_state");
    state.next_start_by_provider.clear();
    state.inflight_by_api_id.clear();
    state.quality_by_api_id.clear();
}

fn normalize_provider_type_value(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "claude" | "anthropic" | "anthropic_native" | "claude_code" => {
            AGGREGATE_API_PROVIDER_CLAUDE.to_string()
        }
        _ => AGGREGATE_API_PROVIDER_CODEX.to_string(),
    }
}

fn first_upstream_header(headers: &reqwest::header::HeaderMap, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        headers
            .get(*name)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn aggregate_api_failure_message(
    status_code: u16,
    body: &[u8],
    request_id: Option<&str>,
    cf_ray: Option<&str>,
    auth_error: Option<&str>,
    identity_error_code: Option<&str>,
) -> String {
    let mut parts =
        vec![
            crate::gateway::summarize_upstream_error_hint_from_body(status_code, body)
                .unwrap_or_else(|| format!("aggregate api upstream status={status_code}")),
        ];
    if let Some(request_id) = request_id.map(str::trim).filter(|value| !value.is_empty()) {
        parts.push(format!("request_id={request_id}"));
    }
    if let Some(cf_ray) = cf_ray.map(str::trim).filter(|value| !value.is_empty()) {
        parts.push(format!("cf_ray={cf_ray}"));
    }
    if let Some(auth_error) = auth_error.map(str::trim).filter(|value| !value.is_empty()) {
        parts.push(format!("auth_error={auth_error}"));
    }
    if let Some(identity_error_code) = identity_error_code
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("identity_error_code={identity_error_code}"));
    }
    if parts.len() == 1 {
        parts.remove(0)
    } else {
        format!("{} [{}]", parts.remove(0), parts.join(", "))
    }
}

fn should_retry_unversioned_aggregate_api_url(status_code: u16) -> bool {
    matches!(status_code, 400 | 404)
}

fn aggregate_api_attempt_urls(
    base_url: &str,
    request_path: &str,
) -> Result<Vec<reqwest::Url>, String> {
    let primary_url = build_aggregate_api_upstream_url(base_url, request_path)?;
    let mut urls = vec![primary_url.clone()];
    if let Some(fallback_url) =
        build_aggregate_api_unversioned_fallback_url(base_url, request_path)?
    {
        if fallback_url != primary_url {
            urls.push(fallback_url);
        }
    }
    Ok(urls)
}

fn build_aggregate_api_request(
    client: &reqwest::blocking::Client,
    request: &Request,
    method: &reqwest::Method,
    url: reqwest::Url,
    body: &Bytes,
    secret: &str,
    request_deadline: Option<Instant>,
    is_stream: bool,
) -> Result<reqwest::blocking::RequestBuilder, String> {
    let mut builder = client.request(method.clone(), url);
    if let Some(timeout) =
        super::super::support::deadline::send_timeout(request_deadline, is_stream)
    {
        builder = builder.timeout(timeout);
    }
    let request_headers = request.headers().to_vec();
    for header in &request_headers {
        if should_skip_forward_header(header.field.as_str().into()) {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(header.field.as_str().as_bytes()),
            HeaderValue::from_str(header.value.as_str()),
        ) {
            builder = builder.header(name, value);
        }
    }
    builder = builder.header(
        HeaderName::from_static("authorization"),
        HeaderValue::from_str(format!("Bearer {}", secret).as_str())
            .map_err(|_| "invalid aggregate api secret".to_string())?,
    );
    if !body.is_empty() {
        builder = builder.body(body.clone());
    }
    Ok(builder)
}

pub(crate) fn resolve_aggregate_api_rotation_candidates(
    storage: &Storage,
    protocol_type: &str,
    aggregate_api_id: Option<&str>,
) -> Result<Vec<AggregateApi>, String> {
    let provider_type = if protocol_type == "anthropic_native" {
        AGGREGATE_API_PROVIDER_CLAUDE
    } else {
        AGGREGATE_API_PROVIDER_CODEX
    };

    let mut candidates = storage
        .list_aggregate_apis()
        .map_err(|err| err.to_string())?
        .into_iter()
        .filter(|api| {
            api.status == "active"
                && normalize_provider_type_value(api.provider_type.as_str()) == provider_type
        })
        .collect::<Vec<_>>();
    candidates = normalize_candidate_order(candidates);
    let mut explicit_preferred = false;

    if let Some(api_id) = aggregate_api_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(preferred) = storage
            .find_aggregate_api_by_id(api_id)
            .map_err(|err| err.to_string())?
        {
            candidates.retain(|api| api.id != preferred.id);
            candidates.insert(0, preferred);
            explicit_preferred = true;
        }
    }

    if !explicit_preferred {
        smart_balance_aggregate_api_candidates(&mut candidates, provider_type);
    }

    if candidates.is_empty() {
        Err(format!(
            "aggregate api not found for provider {provider_type}"
        ))
    } else {
        Ok(candidates)
    }
}

pub(in super::super) struct AggregateProxyExhausted {
    pub(in super::super) request: Request,
    pub(in super::super) attempted_aggregate_api_ids: Vec<String>,
    pub(in super::super) last_attempt_url: Option<String>,
    pub(in super::super) last_attempt_supplier_name: Option<String>,
    pub(in super::super) last_attempt_error: Option<String>,
    pub(in super::super) last_failure_status: u16,
}

pub(in super::super) enum AggregateProxyResult {
    Handled,
    Exhausted(AggregateProxyExhausted),
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn try_proxy_aggregate_request(
    request: Request,
    storage: &Storage,
    trace_id: &str,
    key_id: &str,
    original_path: &str,
    path: &str,
    request_method: &str,
    method: &reqwest::Method,
    body: &Bytes,
    is_stream: bool,
    response_adapter: super::super::super::ResponseAdapter,
    model_for_log: Option<&str>,
    reasoning_for_log: Option<&str>,
    aggregate_api_candidates: Vec<AggregateApi>,
    request_deadline: Option<Instant>,
    started_at: Instant,
    attempted_account_ids_for_log: Option<&[String]>,
) -> Result<AggregateProxyResult, String> {
    proxy_aggregate_request_with_policy(
        request,
        storage,
        trace_id,
        key_id,
        original_path,
        path,
        request_method,
        method,
        body,
        is_stream,
        response_adapter,
        model_for_log,
        reasoning_for_log,
        aggregate_api_candidates,
        request_deadline,
        started_at,
        attempted_account_ids_for_log,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn proxy_aggregate_request_with_policy(
    request: Request,
    storage: &Storage,
    trace_id: &str,
    key_id: &str,
    original_path: &str,
    path: &str,
    request_method: &str,
    method: &reqwest::Method,
    body: &Bytes,
    is_stream: bool,
    response_adapter: super::super::super::ResponseAdapter,
    model_for_log: Option<&str>,
    reasoning_for_log: Option<&str>,
    aggregate_api_candidates: Vec<AggregateApi>,
    request_deadline: Option<Instant>,
    started_at: Instant,
    attempted_account_ids_for_log: Option<&[String]>,
    allow_fallback: bool,
) -> Result<AggregateProxyResult, String> {
    if aggregate_api_candidates.is_empty() {
        let exhausted = AggregateProxyExhausted {
            request,
            attempted_aggregate_api_ids: Vec::new(),
            last_attempt_url: None,
            last_attempt_supplier_name: None,
            last_attempt_error: Some("aggregate api not found".to_string()),
            last_failure_status: 404,
        };
        if allow_fallback {
            return Ok(AggregateProxyResult::Exhausted(exhausted));
        }
        finalize_aggregate_proxy_exhausted(
            exhausted,
            storage,
            trace_id,
            key_id,
            original_path,
            path,
            request_method,
            response_adapter,
            model_for_log,
            reasoning_for_log,
            attempted_account_ids_for_log,
            started_at,
        )?;
        return Ok(AggregateProxyResult::Handled);
    }

    let client = super::super::super::fresh_upstream_client();
    let mut request = Some(request);
    let mut attempted_aggregate_api_ids = Vec::new();
    let mut last_attempt_url: Option<String> = None;
    let mut last_attempt_supplier_name: Option<String> = None;
    let mut last_attempt_error: Option<String> = None;
    let mut last_failure_status = 502u16;

    let total_candidates = aggregate_api_candidates.len();
    for (candidate_idx, candidate) in aggregate_api_candidates.into_iter().enumerate() {
        attempted_aggregate_api_ids.push(candidate.id.clone());
        let candidate_supplier_name = candidate.supplier_name.clone();
        let candidate_url = candidate.url.clone();
        let Some(secret) = storage
            .find_aggregate_api_secret_by_id(candidate.id.as_str())
            .map_err(|err| err.to_string())?
        else {
            last_attempt_url = Some(candidate_url.clone());
            last_attempt_supplier_name = candidate_supplier_name.clone();
            last_attempt_error = Some("aggregate api secret not found".to_string());
            last_failure_status = 403;
            record_aggregate_api_quality(candidate.id.as_str(), 403);
            continue;
        };

        let mut succeeded = false;
        'attempts: for attempt_idx in 0..=AGGREGATE_API_RETRY_ATTEMPTS_PER_CHANNEL {
            if super::super::support::deadline::is_expired(request_deadline) {
                let message = "aggregate api request timeout".to_string();
                let request = request
                    .take()
                    .expect("request should still be available for timeout response");
                super::super::super::record_gateway_request_outcome(
                    path,
                    504,
                    Some("aggregate_api"),
                );
                record_aggregate_api_quality(candidate.id.as_str(), 504);
                super::super::super::trace_log::log_request_final(
                    trace_id,
                    504,
                    Some(key_id),
                    Some(candidate_url.as_str()),
                    Some(message.as_str()),
                    started_at.elapsed().as_millis(),
                );
                super::super::super::request_log::write_request_log_with_attempts(
                    storage,
                    super::super::super::request_log::RequestLogTraceContext {
                        trace_id: Some(trace_id),
                        original_path: Some(original_path),
                        adapted_path: Some(path),
                        response_adapter: Some(response_adapter),
                        aggregate_api_supplier_name: candidate_supplier_name.as_deref(),
                        aggregate_api_url: Some(candidate_url.as_str()),
                        attempted_aggregate_api_ids: Some(attempted_aggregate_api_ids.as_slice()),
                        ..Default::default()
                    },
                    Some(key_id),
                    None,
                    path,
                    request_method,
                    model_for_log,
                    reasoning_for_log,
                    Some(candidate_url.as_str()),
                    Some(504),
                    RequestLogUsage::default(),
                    Some(message.as_str()),
                    Some(started_at.elapsed().as_millis()),
                    attempted_account_ids_for_log,
                );
                respond_error(request, 504, message.as_str(), Some(trace_id));
                return Ok(AggregateProxyResult::Handled);
            }

            let attempt_urls = match aggregate_api_attempt_urls(candidate_url.as_str(), path) {
                Ok(urls) => urls,
                Err(_) => {
                    last_attempt_url = Some(candidate_url.clone());
                    last_attempt_supplier_name = candidate_supplier_name.clone();
                    last_attempt_error = Some("invalid aggregate api url".to_string());
                    last_failure_status = 502;
                    break;
                }
            };
            let total_attempt_urls = attempt_urls.len();
            for (url_idx, url) in attempt_urls.into_iter().enumerate() {
                let builder = build_aggregate_api_request(
                    &client,
                    request.as_ref().expect("request should still be available"),
                    method,
                    url.clone(),
                    body,
                    secret.as_str(),
                    request_deadline,
                    is_stream,
                )?;
                let mut aggregate_inflight_guard =
                    Some(acquire_aggregate_api_inflight(candidate.id.as_str()));

                let attempt_started_at = Instant::now();
                let upstream = match builder.send() {
                    Ok(resp) => {
                        let duration_ms =
                            super::super::super::duration_to_millis(attempt_started_at.elapsed());
                        super::super::super::metrics::record_gateway_upstream_attempt(
                            duration_ms,
                            false,
                        );
                        resp
                    }
                    Err(err) => {
                        let duration_ms =
                            super::super::super::duration_to_millis(attempt_started_at.elapsed());
                        super::super::super::metrics::record_gateway_upstream_attempt(
                            duration_ms,
                            true,
                        );
                        let message = format!("aggregate api upstream error: {err}");
                        last_attempt_url = Some(url.as_str().to_string());
                        last_attempt_supplier_name = candidate_supplier_name.clone();
                        last_attempt_error = Some(message);
                        last_failure_status = 502;
                        record_aggregate_api_quality(candidate.id.as_str(), 502);
                        if url_idx + 1 < total_attempt_urls {
                            continue;
                        }
                        if attempt_idx < AGGREGATE_API_RETRY_ATTEMPTS_PER_CHANNEL {
                            continue 'attempts;
                        }
                        break 'attempts;
                    }
                };

                if !upstream.status().is_success() {
                    let status_code = upstream.status().as_u16();
                    let upstream_request_id = first_upstream_header(
                        upstream.headers(),
                        &["x-request-id", "x-oai-request-id"],
                    );
                    let upstream_cf_ray = first_upstream_header(upstream.headers(), &["cf-ray"]);
                    let upstream_auth_error = first_upstream_header(
                        upstream.headers(),
                        &["x-openai-authorization-error"],
                    );
                    let upstream_identity_error_code =
                        crate::gateway::extract_identity_error_code_from_headers(
                            upstream.headers(),
                        );
                    let upstream_body = upstream
                        .bytes()
                        .map_err(|err| format!("read upstream body failed: {err}"))?;
                    let message = aggregate_api_failure_message(
                        status_code,
                        upstream_body.as_ref(),
                        upstream_request_id.as_deref(),
                        upstream_cf_ray.as_deref(),
                        upstream_auth_error.as_deref(),
                        upstream_identity_error_code.as_deref(),
                    );
                    last_attempt_url = Some(url.as_str().to_string());
                    last_attempt_supplier_name = candidate_supplier_name.clone();
                    last_attempt_error = Some(message);
                    last_failure_status = 502;
                    record_aggregate_api_quality(candidate.id.as_str(), status_code);
                    if url_idx + 1 < total_attempt_urls
                        && should_retry_unversioned_aggregate_api_url(status_code)
                    {
                        continue;
                    }
                    if attempt_idx < AGGREGATE_API_RETRY_ATTEMPTS_PER_CHANNEL {
                        continue 'attempts;
                    }
                    break 'attempts;
                }

                let _aggregate_inflight_guard = aggregate_inflight_guard
                    .take()
                    .expect("aggregate inflight guard should exist");
                let inflight_guard = super::super::super::acquire_account_inflight(key_id);
                let bridge = super::super::super::respond_with_upstream(
                    request
                        .take()
                        .expect("request should be available before bridge"),
                    upstream,
                    inflight_guard,
                    response_adapter,
                    path,
                    None,
                    is_stream,
                    Some(trace_id),
                )
                .map_err(|err| {
                    record_aggregate_api_quality(candidate.id.as_str(), 502);
                    err
                })?;
                let bridge_output_text_len = bridge
                    .usage
                    .output_text
                    .as_deref()
                    .map(str::trim)
                    .map(str::len)
                    .unwrap_or(0);
                super::super::super::trace_log::log_bridge_result(
                    trace_id,
                    format!("{response_adapter:?}").as_str(),
                    path,
                    is_stream,
                    bridge.stream_terminal_seen,
                    bridge.stream_terminal_error.as_deref(),
                    bridge.delivery_error.as_deref(),
                    bridge_output_text_len,
                    bridge.usage.output_tokens,
                    bridge.delivered_status_code,
                    bridge.upstream_error_hint.as_deref(),
                    bridge.upstream_request_id.as_deref(),
                    bridge.upstream_cf_ray.as_deref(),
                    bridge.upstream_auth_error.as_deref(),
                    bridge.upstream_identity_error_code.as_deref(),
                    bridge.upstream_content_type.as_deref(),
                    bridge.last_sse_event_type.as_deref(),
                );
                let bridge_ok = bridge.is_ok(is_stream);
                let mut final_error = bridge.upstream_error_hint.clone();
                if final_error.is_none() && !bridge_ok {
                    final_error = Some(bridge.error_message(is_stream).unwrap_or_else(|| {
                        "aggregate api upstream response incomplete".to_string()
                    }));
                }
                let status_code =
                    bridge
                        .delivered_status_code
                        .unwrap_or_else(|| if bridge_ok { 200 } else { 502 });
                let status_code = if final_error.is_some() && status_code < 400 {
                    502
                } else {
                    status_code
                };
                record_aggregate_api_quality(candidate.id.as_str(), status_code);
                let usage = bridge.usage;

                super::super::super::record_gateway_request_outcome(
                    path,
                    status_code,
                    Some("aggregate_api"),
                );
                super::super::super::trace_log::log_request_final(
                    trace_id,
                    status_code,
                    Some(key_id),
                    Some(url.as_str()),
                    final_error.as_deref(),
                    started_at.elapsed().as_millis(),
                );
                super::super::super::request_log::write_request_log_with_attempts(
                    storage,
                    super::super::super::request_log::RequestLogTraceContext {
                        trace_id: Some(trace_id),
                        original_path: Some(original_path),
                        adapted_path: Some(path),
                        response_adapter: Some(response_adapter),
                        aggregate_api_supplier_name: candidate_supplier_name.as_deref(),
                        aggregate_api_url: Some(candidate_url.as_str()),
                        attempted_aggregate_api_ids: Some(attempted_aggregate_api_ids.as_slice()),
                        ..Default::default()
                    },
                    Some(key_id),
                    None,
                    path,
                    request_method,
                    model_for_log,
                    reasoning_for_log,
                    Some(url.as_str()),
                    Some(status_code),
                    RequestLogUsage {
                        input_tokens: usage.input_tokens,
                        cached_input_tokens: usage.cached_input_tokens,
                        output_tokens: usage.output_tokens,
                        total_tokens: usage.total_tokens,
                        reasoning_output_tokens: usage.reasoning_output_tokens,
                    },
                    final_error.as_deref(),
                    Some(started_at.elapsed().as_millis()),
                    attempted_account_ids_for_log,
                );
                succeeded = true;
                break 'attempts;
            }
        }

        if succeeded {
            return Ok(AggregateProxyResult::Handled);
        }

        if candidate_idx + 1 < total_candidates {
            super::super::super::record_gateway_failover_attempt();
        }
    }

    let exhausted = AggregateProxyExhausted {
        request: request
            .take()
            .expect("request should still be available for failure response"),
        attempted_aggregate_api_ids,
        last_attempt_url,
        last_attempt_supplier_name,
        last_attempt_error,
        last_failure_status,
    };
    if allow_fallback {
        Ok(AggregateProxyResult::Exhausted(exhausted))
    } else {
        finalize_aggregate_proxy_exhausted(
            exhausted,
            storage,
            trace_id,
            key_id,
            original_path,
            path,
            request_method,
            response_adapter,
            model_for_log,
            reasoning_for_log,
            attempted_account_ids_for_log,
            started_at,
        )?;
        Ok(AggregateProxyResult::Handled)
    }
}

#[allow(clippy::too_many_arguments)]
fn finalize_aggregate_proxy_exhausted(
    exhausted: AggregateProxyExhausted,
    storage: &Storage,
    trace_id: &str,
    key_id: &str,
    original_path: &str,
    path: &str,
    request_method: &str,
    response_adapter: super::super::super::ResponseAdapter,
    model_for_log: Option<&str>,
    reasoning_for_log: Option<&str>,
    attempted_account_ids_for_log: Option<&[String]>,
    started_at: Instant,
) -> Result<(), String> {
    let message = exhausted
        .last_attempt_error
        .clone()
        .unwrap_or_else(|| "aggregate api upstream response failed".to_string());
    let status_code = exhausted.last_failure_status;
    super::super::super::record_gateway_request_outcome(path, status_code, Some("aggregate_api"));
    super::super::super::trace_log::log_request_final(
        trace_id,
        status_code,
        Some(key_id),
        exhausted.last_attempt_url.as_deref(),
        Some(message.as_str()),
        started_at.elapsed().as_millis(),
    );
    super::super::super::request_log::write_request_log_with_attempts(
        storage,
        super::super::super::request_log::RequestLogTraceContext {
            trace_id: Some(trace_id),
            original_path: Some(original_path),
            adapted_path: Some(path),
            response_adapter: Some(response_adapter),
            aggregate_api_supplier_name: exhausted.last_attempt_supplier_name.as_deref(),
            aggregate_api_url: exhausted.last_attempt_url.as_deref(),
            attempted_aggregate_api_ids: Some(exhausted.attempted_aggregate_api_ids.as_slice()),
            ..Default::default()
        },
        Some(key_id),
        None,
        path,
        request_method,
        model_for_log,
        reasoning_for_log,
        exhausted.last_attempt_url.as_deref(),
        Some(status_code),
        RequestLogUsage::default(),
        Some(message.as_str()),
        Some(started_at.elapsed().as_millis()),
        attempted_account_ids_for_log,
    );
    respond_error(
        exhausted.request,
        status_code,
        message.as_str(),
        Some(trace_id),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        acquire_aggregate_api_inflight, aggregate_api_attempt_urls, clear_runtime_state,
        record_aggregate_api_quality, resolve_aggregate_api_rotation_candidates,
        should_retry_unversioned_aggregate_api_url,
    };
    use codexmanager_core::storage::{now_ts, AggregateApi, Storage};
    use std::sync::{Mutex, OnceLock};

    fn aggregate_api_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static AGGREGATE_API_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
        AGGREGATE_API_TEST_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn final_error_promotes_success_status_to_bad_gateway() {
        let status_code = bridge_status_code(Some(200), true, Some("unsupported model"));
        assert_eq!(status_code, 502);
    }

    #[test]
    fn successful_bridge_keeps_success_status() {
        let status_code = bridge_status_code(Some(200), true, None);
        assert_eq!(status_code, 200);
    }

    #[test]
    fn incomplete_bridge_without_status_defaults_to_bad_gateway() {
        let status_code = bridge_status_code(None, false, None);
        assert_eq!(status_code, 502);
    }

    #[test]
    fn aggregate_api_attempt_urls_adds_unversioned_fallback_for_prefixed_openai_urls() {
        let urls =
            aggregate_api_attempt_urls("https://fizzlycode.com/openai", "/v1/responses?trace=1")
                .expect("attempt urls");

        let actual = urls
            .into_iter()
            .map(|url| url.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![
                "https://fizzlycode.com/openai/v1/responses?trace=1".to_string(),
                "https://fizzlycode.com/openai/responses?trace=1".to_string(),
            ]
        );
    }

    #[test]
    fn unversioned_fallback_retry_is_limited_to_path_mismatch_statuses() {
        assert!(should_retry_unversioned_aggregate_api_url(400));
        assert!(should_retry_unversioned_aggregate_api_url(404));
        assert!(!should_retry_unversioned_aggregate_api_url(401));
        assert!(!should_retry_unversioned_aggregate_api_url(429));
    }

    #[test]
    fn aggregate_api_candidates_round_robin_when_runtime_scores_are_equal() {
        let _guard = aggregate_api_test_guard();
        clear_runtime_state();
        let storage = Storage::open_in_memory().expect("open");
        storage.init().expect("init");
        let now = now_ts();
        for (idx, id) in ["agg-a", "agg-b", "agg-c"].into_iter().enumerate() {
            storage
                .insert_aggregate_api(&AggregateApi {
                    id: id.to_string(),
                    provider_type: "codex".to_string(),
                    supplier_name: Some(id.to_string()),
                    sort: idx as i64,
                    url: format!("https://example.com/{id}"),
                    status: "active".to_string(),
                    created_at: now + idx as i64,
                    updated_at: now + idx as i64,
                    last_test_at: None,
                    last_test_status: None,
                    last_test_error: None,
                })
                .expect("insert aggregate api");
        }

        let first = resolve_aggregate_api_rotation_candidates(&storage, "openai_compat", None)
            .expect("first candidates");
        let second = resolve_aggregate_api_rotation_candidates(&storage, "openai_compat", None)
            .expect("second candidates");
        let third = resolve_aggregate_api_rotation_candidates(&storage, "openai_compat", None)
            .expect("third candidates");

        assert_eq!(first[0].id, "agg-a");
        assert_eq!(second[0].id, "agg-b");
        assert_eq!(third[0].id, "agg-c");
    }

    #[test]
    fn aggregate_api_candidates_prefer_healthier_and_less_busy_entries() {
        let _guard = aggregate_api_test_guard();
        clear_runtime_state();
        let storage = Storage::open_in_memory().expect("open");
        storage.init().expect("init");
        let now = now_ts();
        storage
            .insert_aggregate_api(&AggregateApi {
                id: "agg-busy".to_string(),
                provider_type: "codex".to_string(),
                supplier_name: Some("busy".to_string()),
                sort: 0,
                url: "https://example.com/busy".to_string(),
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
                last_test_at: None,
                last_test_status: Some("failed".to_string()),
                last_test_error: None,
            })
            .expect("insert busy aggregate api");
        storage
            .insert_aggregate_api(&AggregateApi {
                id: "agg-healthy".to_string(),
                provider_type: "codex".to_string(),
                supplier_name: Some("healthy".to_string()),
                sort: 1,
                url: "https://example.com/healthy".to_string(),
                status: "active".to_string(),
                created_at: now + 1,
                updated_at: now + 1,
                last_test_at: None,
                last_test_status: Some("success".to_string()),
                last_test_error: None,
            })
            .expect("insert healthy aggregate api");

        for _ in 0..4 {
            record_aggregate_api_quality("agg-busy", 429);
            record_aggregate_api_quality("agg-healthy", 200);
        }
        let _busy_guard = acquire_aggregate_api_inflight("agg-busy");

        let candidates = resolve_aggregate_api_rotation_candidates(&storage, "openai_compat", None)
            .expect("candidates");
        assert_eq!(candidates[0].id, "agg-healthy");
    }

    fn bridge_status_code(
        delivered_status_code: Option<u16>,
        bridge_ok: bool,
        final_error: Option<&str>,
    ) -> u16 {
        let status_code =
            delivered_status_code.unwrap_or_else(|| if bridge_ok { 200 } else { 502 });
        if final_error.is_some() && status_code < 400 {
            502
        } else {
            status_code
        }
    }
}
