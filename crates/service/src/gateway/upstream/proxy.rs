use crate::apikey_profile::ROTATION_AGGREGATE_API;
use crate::apikey_profile::{PROTOCOL_ANTHROPIC_NATIVE, PROTOCOL_AZURE_OPENAI};
use crate::gateway::request_log::RequestLogUsage;
use std::time::Instant;
use tiny_http::Request;

use super::super::local_validation::{LocalValidationResult, PreparedGatewayRequest};
use super::protocol::aggregate_api::{
    try_proxy_aggregate_request, AggregateProxyExhausted, AggregateProxyResult,
};
use super::proxy_pipeline::candidate_executor::{
    execute_candidate_sequence, CandidateExecutionResult, CandidateExecutorParams,
};
use super::proxy_pipeline::execution_context::GatewayUpstreamExecutionContext;
use super::proxy_pipeline::request_gate::acquire_request_gate;
use super::proxy_pipeline::request_setup::prepare_request_setup;
use super::proxy_pipeline::response_finalize::respond_terminal;
use super::support::candidates::prepare_gateway_candidates;

struct AccountProxyExhausted {
    request: Request,
    attempted_account_ids: Vec<String>,
    skipped_cooldown: usize,
    skipped_inflight: usize,
    last_attempt_url: Option<String>,
    last_attempt_error: Option<String>,
}

enum AccountProxyResult {
    Handled,
    Exhausted(AccountProxyExhausted),
}

fn exhausted_gateway_error_for_log(
    attempted_account_ids: &[String],
    skipped_cooldown: usize,
    skipped_inflight: usize,
    last_attempt_error: Option<&str>,
) -> String {
    let kind = if !attempted_account_ids.is_empty() {
        "no_available_account_exhausted"
    } else if skipped_cooldown > 0 && skipped_inflight > 0 {
        "no_available_account_skipped"
    } else if skipped_cooldown > 0 {
        "no_available_account_cooldown"
    } else if skipped_inflight > 0 {
        "no_available_account_inflight"
    } else {
        "no_available_account"
    };
    let mut parts = vec!["no available account".to_string(), format!("kind={kind}")];
    if !attempted_account_ids.is_empty() {
        parts.push(format!("attempted={}", attempted_account_ids.join(",")));
    }
    if skipped_cooldown > 0 || skipped_inflight > 0 {
        parts.push(format!(
            "skipped(cooldown={}, inflight={})",
            skipped_cooldown, skipped_inflight
        ));
    }
    if let Some(last_attempt_error) = last_attempt_error
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("last_attempt={last_attempt_error}"));
    }
    parts.join("; ")
}

fn aggregate_exhausted_error_for_log(exhausted: &AggregateProxyExhausted) -> String {
    let kind = if exhausted.attempted_aggregate_api_ids.is_empty() {
        "aggregate_api_unavailable"
    } else {
        "aggregate_api_exhausted"
    };
    let mut parts = vec![
        "no available aggregate api".to_string(),
        format!("kind={kind}"),
    ];
    if !exhausted.attempted_aggregate_api_ids.is_empty() {
        parts.push(format!(
            "attempted={}",
            exhausted.attempted_aggregate_api_ids.join(",")
        ));
    }
    if let Some(url) = exhausted
        .last_attempt_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("last_url={url}"));
    }
    if let Some(last_attempt_error) = exhausted
        .last_attempt_error
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("last_attempt={last_attempt_error}"));
    }
    parts.join("; ")
}

fn request_uses_upstream_sse(path: &str, client_is_stream: bool) -> bool {
    let is_compact_path =
        path == "/v1/responses/compact" || path.starts_with("/v1/responses/compact?");
    client_is_stream || (path.starts_with("/v1/responses") && !is_compact_path)
}

#[allow(clippy::too_many_arguments)]
fn log_and_respond_terminal_failure(
    request: Request,
    validated: &LocalValidationResult,
    prepared: &PreparedGatewayRequest,
    started_at: Instant,
    outcome_protocol: Option<&str>,
    aggregate_api_supplier_name: Option<&str>,
    aggregate_api_url: Option<&str>,
    attempted_account_ids: Option<&[String]>,
    attempted_aggregate_api_ids: Option<&[String]>,
    upstream_url: Option<&str>,
    status_code: u16,
    error_for_log: &str,
    response_message: &str,
) -> Result<(), String> {
    super::super::request_log::write_request_log_with_attempts(
        &validated.storage,
        super::super::request_log::RequestLogTraceContext {
            trace_id: Some(validated.trace_id.as_str()),
            original_path: Some(validated.original_path.as_str()),
            adapted_path: Some(prepared.path.as_str()),
            response_adapter: Some(prepared.response_adapter),
            aggregate_api_supplier_name,
            aggregate_api_url,
            attempted_aggregate_api_ids,
        },
        Some(validated.key_id.as_str()),
        None,
        prepared.path.as_str(),
        validated.request_method.as_str(),
        prepared.model_for_log.as_deref(),
        prepared.reasoning_for_log.as_deref(),
        upstream_url,
        Some(status_code),
        RequestLogUsage::default(),
        Some(error_for_log),
        Some(started_at.elapsed().as_millis()),
        attempted_account_ids,
    );
    super::super::trace_log::log_request_final(
        validated.trace_id.as_str(),
        status_code,
        Some(validated.key_id.as_str()),
        upstream_url,
        Some(error_for_log),
        started_at.elapsed().as_millis(),
    );
    super::super::record_gateway_request_outcome(
        prepared.path.as_str(),
        status_code,
        outcome_protocol,
    );
    respond_terminal(
        request,
        status_code,
        response_message.to_string(),
        Some(validated.trace_id.as_str()),
    )
}

fn aggregate_not_found_exhausted(request: Request, message: String) -> AggregateProxyExhausted {
    AggregateProxyExhausted {
        request,
        attempted_aggregate_api_ids: Vec::new(),
        last_attempt_url: None,
        last_attempt_supplier_name: None,
        last_attempt_error: Some(message),
        last_failure_status: 404,
    }
}

#[allow(clippy::too_many_arguments)]
fn try_resolve_and_proxy_aggregate_request(
    request: Request,
    validated: &LocalValidationResult,
    prepared: &PreparedGatewayRequest,
    request_deadline: Option<Instant>,
    started_at: Instant,
    attempted_account_ids_for_log: Option<&[String]>,
) -> Result<AggregateProxyResult, String> {
    let aggregate_api_candidates =
        match super::protocol::aggregate_api::resolve_aggregate_api_rotation_candidates(
            &validated.storage,
            validated.protocol_type.as_str(),
            validated.aggregate_api_id.as_deref(),
        ) {
            Ok(candidates) => candidates,
            Err(err) => {
                return Ok(AggregateProxyResult::Exhausted(
                    aggregate_not_found_exhausted(request, err),
                ));
            }
        };

    try_proxy_aggregate_request(
        request,
        &validated.storage,
        validated.trace_id.as_str(),
        validated.key_id.as_str(),
        validated.original_path.as_str(),
        prepared.path.as_str(),
        validated.request_method.as_str(),
        &validated.method,
        &prepared.body,
        validated.is_stream,
        prepared.response_adapter,
        prepared.model_for_log.as_deref(),
        prepared.reasoning_for_log.as_deref(),
        aggregate_api_candidates,
        request_deadline,
        started_at,
        attempted_account_ids_for_log,
    )
}

#[allow(clippy::too_many_arguments)]
fn try_proxy_account_request(
    request: Request,
    validated: &LocalValidationResult,
    prepared: &PreparedGatewayRequest,
    request_deadline: Option<Instant>,
    started_at: Instant,
    debug: bool,
    attempted_aggregate_api_ids_for_log: Option<&[String]>,
) -> Result<AccountProxyResult, String> {
    let upstream_is_stream = request_uses_upstream_sse(prepared.path.as_str(), validated.is_stream);

    if validated.protocol_type == PROTOCOL_AZURE_OPENAI {
        super::protocol::azure_openai::proxy_azure_request(
            request,
            &validated.storage,
            validated.trace_id.as_str(),
            validated.key_id.as_str(),
            validated.original_path.as_str(),
            prepared.path.as_str(),
            validated.request_method.as_str(),
            &validated.method,
            &prepared.body,
            upstream_is_stream,
            prepared.response_adapter,
            &prepared.tool_name_restore_map,
            prepared.model_for_log.as_deref(),
            prepared.reasoning_for_log.as_deref(),
            validated.upstream_base_url.as_deref(),
            validated.static_headers_json.as_deref(),
            request_deadline,
            started_at,
        )?;
        return Ok(AccountProxyResult::Handled);
    }

    let mut candidates =
        match prepare_gateway_candidates(&validated.storage, prepared.model_for_log.as_deref()) {
            Ok(candidates) => candidates,
            Err(err) => {
                let err_text = format!("candidate resolve failed: {err}");
                log_and_respond_terminal_failure(
                    request,
                    validated,
                    prepared,
                    started_at,
                    Some(validated.protocol_type.as_str()),
                    None,
                    None,
                    None,
                    attempted_aggregate_api_ids_for_log,
                    None,
                    500,
                    err_text.as_str(),
                    err_text.as_str(),
                )?;
                return Ok(AccountProxyResult::Handled);
            }
        };

    if candidates.is_empty() {
        return Ok(AccountProxyResult::Exhausted(AccountProxyExhausted {
            request,
            attempted_account_ids: Vec::new(),
            skipped_cooldown: 0,
            skipped_inflight: 0,
            last_attempt_url: None,
            last_attempt_error: None,
        }));
    }

    let setup = prepare_request_setup(
        prepared.path.as_str(),
        validated.protocol_type.as_str(),
        prepared.has_prompt_cache_key,
        &validated.incoming_headers,
        &prepared.body,
        &mut candidates,
        validated.key_id.as_str(),
        validated.platform_key_hash.as_str(),
        prepared.local_conversation_id.as_deref(),
        prepared.conversation_binding.as_ref(),
        prepared.model_for_log.as_deref(),
        validated.trace_id.as_str(),
    );
    let context = GatewayUpstreamExecutionContext::new(
        validated.trace_id.as_str(),
        &validated.storage,
        validated.key_id.as_str(),
        validated.original_path.as_str(),
        prepared.path.as_str(),
        validated.request_method.as_str(),
        prepared.response_adapter,
        validated.protocol_type.as_str(),
        prepared.reasoning_for_log.as_deref(),
        setup.candidate_count,
        setup.account_max_inflight,
        attempted_aggregate_api_ids_for_log,
    );
    let allow_openai_fallback = false;
    let disable_challenge_stateless_retry = !(validated.protocol_type == PROTOCOL_ANTHROPIC_NATIVE
        && prepared.body.len() <= 2 * 1024)
        && !prepared.path.starts_with("/v1/responses");
    let _request_gate_guard = acquire_request_gate(
        validated.trace_id.as_str(),
        validated.key_id.as_str(),
        prepared.path.as_str(),
        prepared.model_for_log.as_deref(),
        request_deadline,
    );
    match execute_candidate_sequence(
        request,
        candidates,
        CandidateExecutorParams {
            storage: &validated.storage,
            method: &validated.method,
            incoming_headers: &validated.incoming_headers,
            body: &prepared.body,
            path: prepared.path.as_str(),
            request_shape: prepared.request_shape.as_deref(),
            trace_id: validated.trace_id.as_str(),
            model_for_log: prepared.model_for_log.as_deref(),
            response_adapter: prepared.response_adapter,
            tool_name_restore_map: &prepared.tool_name_restore_map,
            context: &context,
            setup: &setup,
            request_deadline,
            started_at,
            client_is_stream: validated.is_stream,
            upstream_is_stream,
            debug,
            allow_openai_fallback,
            disable_challenge_stateless_retry,
        },
    )? {
        CandidateExecutionResult::Handled => Ok(AccountProxyResult::Handled),
        CandidateExecutionResult::Exhausted {
            request,
            attempted_account_ids,
            skipped_cooldown,
            skipped_inflight,
            last_attempt_url,
            last_attempt_error,
        } => Ok(AccountProxyResult::Exhausted(AccountProxyExhausted {
            request,
            attempted_account_ids,
            skipped_cooldown,
            skipped_inflight,
            last_attempt_url,
            last_attempt_error,
        })),
    }
}

pub(in super::super) fn proxy_validated_request(
    request: Request,
    validated: LocalValidationResult,
    debug: bool,
) -> Result<(), String> {
    let started_at = Instant::now();
    let request_deadline =
        super::support::deadline::request_deadline(started_at, validated.is_stream);
    let primary_request = validated.primary_request();

    super::super::trace_log::log_request_start(
        validated.trace_id.as_str(),
        validated.key_id.as_str(),
        validated.request_method.as_str(),
        primary_request.path.as_str(),
        primary_request.model_for_log.as_deref(),
        primary_request.reasoning_for_log.as_deref(),
        validated.is_stream,
        validated.protocol_type.as_str(),
    );
    super::super::trace_log::log_request_body_preview(
        validated.trace_id.as_str(),
        primary_request.body.as_ref(),
    );

    if validated.rotation_strategy == ROTATION_AGGREGATE_API {
        let aggregate_prepared = validated.aggregate_request();
        let aggregate_exhausted = match try_resolve_and_proxy_aggregate_request(
            request,
            &validated,
            aggregate_prepared,
            request_deadline,
            started_at,
            None,
        )? {
            AggregateProxyResult::Handled => return Ok(()),
            AggregateProxyResult::Exhausted(exhausted) => exhausted,
        };
        let aggregate_error = aggregate_exhausted_error_for_log(&aggregate_exhausted);
        let AggregateProxyExhausted {
            request,
            attempted_aggregate_api_ids,
            last_attempt_url: aggregate_last_attempt_url,
            last_attempt_supplier_name,
            last_attempt_error: _,
            last_failure_status: _,
        } = aggregate_exhausted;

        let account_prepared = validated.account_request();
        match try_proxy_account_request(
            request,
            &validated,
            account_prepared,
            request_deadline,
            started_at,
            debug,
            (!attempted_aggregate_api_ids.is_empty())
                .then_some(attempted_aggregate_api_ids.as_slice()),
        )? {
            AccountProxyResult::Handled => Ok(()),
            AccountProxyResult::Exhausted(account_exhausted) => {
                let AccountProxyExhausted {
                    request,
                    attempted_account_ids,
                    skipped_cooldown,
                    skipped_inflight,
                    last_attempt_url: account_last_attempt_url,
                    last_attempt_error,
                } = account_exhausted;
                let account_error = exhausted_gateway_error_for_log(
                    attempted_account_ids.as_slice(),
                    skipped_cooldown,
                    skipped_inflight,
                    last_attempt_error.as_deref(),
                );
                let final_error = format!("{aggregate_error}; account_fallback={account_error}");
                log_and_respond_terminal_failure(
                    request,
                    &validated,
                    account_prepared,
                    started_at,
                    Some(validated.protocol_type.as_str()),
                    last_attempt_supplier_name.as_deref(),
                    aggregate_last_attempt_url.as_deref(),
                    (!attempted_account_ids.is_empty()).then_some(attempted_account_ids.as_slice()),
                    (!attempted_aggregate_api_ids.is_empty())
                        .then_some(attempted_aggregate_api_ids.as_slice()),
                    account_last_attempt_url
                        .as_deref()
                        .or(aggregate_last_attempt_url.as_deref()),
                    503,
                    final_error.as_str(),
                    "no available account",
                )
            }
        }
    } else {
        let account_prepared = validated.account_request();
        let account_exhausted = match try_proxy_account_request(
            request,
            &validated,
            account_prepared,
            request_deadline,
            started_at,
            debug,
            None,
        )? {
            AccountProxyResult::Handled => return Ok(()),
            AccountProxyResult::Exhausted(exhausted) => exhausted,
        };
        let account_error = exhausted_gateway_error_for_log(
            account_exhausted.attempted_account_ids.as_slice(),
            account_exhausted.skipped_cooldown,
            account_exhausted.skipped_inflight,
            account_exhausted.last_attempt_error.as_deref(),
        );
        let AccountProxyExhausted {
            request,
            attempted_account_ids,
            skipped_cooldown: _,
            skipped_inflight: _,
            last_attempt_url: account_last_attempt_url,
            last_attempt_error: _,
        } = account_exhausted;

        let aggregate_prepared = validated.aggregate_request();
        match try_resolve_and_proxy_aggregate_request(
            request,
            &validated,
            aggregate_prepared,
            request_deadline,
            started_at,
            (!attempted_account_ids.is_empty()).then_some(attempted_account_ids.as_slice()),
        )? {
            AggregateProxyResult::Handled => Ok(()),
            AggregateProxyResult::Exhausted(aggregate_exhausted) => {
                let aggregate_error = aggregate_exhausted_error_for_log(&aggregate_exhausted);
                let final_error = format!("{account_error}; aggregate_fallback={aggregate_error}");
                let AggregateProxyExhausted {
                    request,
                    attempted_aggregate_api_ids,
                    last_attempt_url,
                    last_attempt_supplier_name,
                    last_attempt_error: _,
                    last_failure_status: _,
                } = aggregate_exhausted;
                log_and_respond_terminal_failure(
                    request,
                    &validated,
                    aggregate_prepared,
                    started_at,
                    Some("aggregate_api"),
                    last_attempt_supplier_name.as_deref(),
                    last_attempt_url.as_deref(),
                    (!attempted_account_ids.is_empty()).then_some(attempted_account_ids.as_slice()),
                    (!attempted_aggregate_api_ids.is_empty())
                        .then_some(attempted_aggregate_api_ids.as_slice()),
                    last_attempt_url
                        .as_deref()
                        .or(account_last_attempt_url.as_deref()),
                    503,
                    final_error.as_str(),
                    "no available account",
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::exhausted_gateway_error_for_log;

    #[test]
    fn exhausted_gateway_error_includes_attempts_skips_and_last_error() {
        let message = exhausted_gateway_error_for_log(
            &["acc-a".to_string(), "acc-b".to_string()],
            2,
            1,
            Some("upstream challenge blocked"),
        );

        assert!(message.contains("no available account"));
        assert!(message.contains("kind=no_available_account_exhausted"));
        assert!(message.contains("attempted=acc-a,acc-b"));
        assert!(message.contains("skipped(cooldown=2, inflight=1)"));
        assert!(message.contains("last_attempt=upstream challenge blocked"));
    }

    #[test]
    fn exhausted_gateway_error_marks_cooldown_only_skip_kind() {
        let message = exhausted_gateway_error_for_log(&[], 2, 0, None);

        assert!(message.contains("kind=no_available_account_cooldown"));
    }
}
