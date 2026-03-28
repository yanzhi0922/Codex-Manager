use codexmanager_core::rpc::types::RequestLogFilterSummaryResult;

use crate::storage_helpers::open_storage;

use super::list::{normalize_optional_text, normalize_status_filter};

pub(crate) fn read_request_log_filter_summary(
    query: Option<String>,
    status_filter: Option<String>,
    aggregate_only: bool,
) -> Result<RequestLogFilterSummaryResult, String> {
    let storage = open_storage().ok_or_else(|| "open storage failed".to_string())?;
    let query = normalize_optional_text(query);
    let status_filter = normalize_status_filter(status_filter);
    let total_count = storage
        .count_request_logs_scoped(query.as_deref(), None, aggregate_only)
        .map_err(|err| format!("count request logs failed: {err}"))?;
    let filtered = storage
        .summarize_request_logs_filtered_scoped(
            query.as_deref(),
            status_filter.as_deref(),
            aggregate_only,
        )
        .map_err(|err| format!("summarize request logs failed: {err}"))?;

    Ok(RequestLogFilterSummaryResult {
        total_count,
        filtered_count: filtered.count,
        success_count: filtered.success_count,
        error_count: filtered.error_count,
        total_tokens: filtered.total_tokens,
    })
}
