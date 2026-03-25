use bytes::Bytes;
use codexmanager_core::storage::Storage;
use reqwest::header::{HeaderName, HeaderValue};
use std::time::Instant;
use tiny_http::Request;

use crate::gateway::request_log::RequestLogUsage;

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

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn proxy_aggregate_request(
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
    upstream_base_url: Option<&str>,
    aggregate_api_secret: Option<&str>,
    request_deadline: Option<Instant>,
    started_at: Instant,
) -> Result<(), String> {
    let Some(base) = upstream_base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        let message = "aggregate api base url missing";
        super::super::super::record_gateway_request_outcome(path, 400, Some("aggregate_api"));
        super::super::super::trace_log::log_request_final(
            trace_id,
            400,
            Some(key_id),
            None,
            Some(message),
            started_at.elapsed().as_millis(),
        );
        respond_error(request, 400, message, Some(trace_id));
        return Ok(());
    };
    let Some(secret) = aggregate_api_secret
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        let message = "aggregate api secret missing";
        super::super::super::record_gateway_request_outcome(path, 403, Some("aggregate_api"));
        super::super::super::trace_log::log_request_final(
            trace_id,
            403,
            Some(key_id),
            None,
            Some(message),
            started_at.elapsed().as_millis(),
        );
        respond_error(request, 403, message, Some(trace_id));
        return Ok(());
    };

    let url = reqwest::Url::parse(base)
        .and_then(|url| url.join(path))
        .map_err(|_| "invalid aggregate api url".to_string())?;
    let client = super::super::super::fresh_upstream_client();
    let mut builder = client.request(method.clone(), url.clone());
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

    let attempt_started_at = Instant::now();
    let upstream = match builder.send() {
        Ok(resp) => {
            let duration_ms = super::super::super::duration_to_millis(attempt_started_at.elapsed());
            super::super::super::metrics::record_gateway_upstream_attempt(duration_ms, false);
            resp
        }
        Err(err) => {
            let duration_ms = super::super::super::duration_to_millis(attempt_started_at.elapsed());
            super::super::super::metrics::record_gateway_upstream_attempt(duration_ms, true);
            let message = format!("aggregate api upstream error: {err}");
            super::super::super::record_gateway_request_outcome(
                path,
                502,
                Some("aggregate_api"),
            );
            super::super::super::trace_log::log_request_final(
                trace_id,
                502,
                Some(key_id),
                Some(url.as_str()),
                Some(message.as_str()),
                started_at.elapsed().as_millis(),
            );
            super::super::super::write_request_log(
                storage,
                super::super::super::request_log::RequestLogTraceContext {
                    trace_id: Some(trace_id),
                    original_path: Some(original_path),
                    adapted_path: Some(path),
                    response_adapter: Some(response_adapter),
                },
                Some(key_id),
                None,
                path,
                request_method,
                model_for_log,
                reasoning_for_log,
                Some(url.as_str()),
                Some(502),
                super::super::super::request_log::RequestLogUsage::default(),
                Some(message.as_str()),
                Some(started_at.elapsed().as_millis()),
            );
            respond_error(request, 502, message.as_str(), Some(trace_id));
            return Ok(());
        }
    };

    let inflight_guard = super::super::super::acquire_account_inflight(key_id);
    let bridge = super::super::super::respond_with_upstream(
        request,
        upstream,
        inflight_guard,
        response_adapter,
        path,
        None,
        is_stream,
        Some(trace_id),
    )?;
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
    let status_code = bridge
        .delivered_status_code
        .unwrap_or_else(|| if bridge_ok { 200 } else { 502 });
    let status_code = if final_error.is_some() && status_code < 400 {
        502
    } else {
        status_code
    };
    let usage = bridge.usage;

    super::super::super::record_gateway_request_outcome(path, status_code, Some("aggregate_api"));
    super::super::super::trace_log::log_request_final(
        trace_id,
        status_code,
        Some(key_id),
        Some(url.as_str()),
        final_error.as_deref(),
        started_at.elapsed().as_millis(),
    );
    super::super::super::write_request_log(
        storage,
        super::super::super::request_log::RequestLogTraceContext {
            trace_id: Some(trace_id),
            original_path: Some(original_path),
            adapted_path: Some(path),
            response_adapter: Some(response_adapter),
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
    );
    Ok(())
}

#[cfg(test)]
mod tests {
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

    fn bridge_status_code(
        delivered_status_code: Option<u16>,
        bridge_ok: bool,
        final_error: Option<&str>,
    ) -> u16 {
        let status_code = delivered_status_code.unwrap_or_else(|| if bridge_ok { 200 } else { 502 });
        if final_error.is_some() && status_code < 400 {
            502
        } else {
            status_code
        }
    }
}
