use codexmanager_core::rpc::types::{JsonRpcRequest, JsonRpcResponse, RequestLogListParams};

use crate::{requestlog_clear, requestlog_list, requestlog_summary, requestlog_today_summary};

pub(super) fn try_handle(req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    let result = match req.method.as_str() {
        "requestlog/list" => {
            let params = req
                .params
                .clone()
                .map(serde_json::from_value::<RequestLogListParams>)
                .transpose()
                .map(|params| params.unwrap_or_default())
                .map(RequestLogListParams::normalized)
                .map_err(|err| format!("invalid requestlog/list params: {err}"));
            super::value_or_error(params.and_then(requestlog_list::read_request_log_page))
        }
        "requestlog/summary" => {
            let query = super::string_param(req, "query");
            let status_filter = super::string_param(req, "statusFilter");
            let aggregate_only = super::bool_param(req, "aggregateOnly").unwrap_or(false);
            super::value_or_error(requestlog_summary::read_request_log_filter_summary(
                query,
                status_filter,
                aggregate_only,
            ))
        }
        "requestlog/clear" => super::ok_or_error(requestlog_clear::clear_request_logs()),
        "requestlog/today_summary" => {
            let aggregate_only = super::bool_param(req, "aggregateOnly").unwrap_or(false);
            super::value_or_error(requestlog_today_summary::read_requestlog_today_summary(
                aggregate_only,
            ))
        }
        _ => return None,
    };

    Some(super::response(req, result))
}
