use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub id: u64,
    pub result: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeResult {
    pub server_name: String,
    pub version: String,
    pub user_agent: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlatformDiscoveryTotals {
    pub ready: i64,
    pub detected: i64,
    pub missing: i64,
    pub planned: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlatformDiscoveryItem {
    pub id: String,
    pub name: String,
    pub category: String,
    pub status: String,
    #[serde(default)]
    pub primary_path: Option<String>,
    #[serde(default)]
    pub detected_paths: Vec<String>,
    #[serde(default)]
    pub signals: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlatformDiscoveryResult {
    #[serde(rename = "generatedAt")]
    pub generated_at: i64,
    pub totals: PlatformDiscoveryTotals,
    pub items: Vec<PlatformDiscoveryItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub id: String,
    pub label: String,
    pub group_name: Option<String>,
    pub sort: i64,
    pub status: String,
    pub status_reason: Option<String>,
    pub plan_type: Option<String>,
    pub plan_type_raw: Option<String>,
    pub note: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AccountListParams {
    pub page: i64,
    pub page_size: i64,
    pub query: Option<String>,
    pub filter: Option<String>,
    pub group_filter: Option<String>,
}

impl Default for AccountListParams {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 5,
            query: None,
            filter: None,
            group_filter: None,
        }
    }
}

impl AccountListParams {
    pub fn normalized(self) -> Self {
        // 中文注释：分页参数小于 1 时回退到默认值，避免出现负偏移或零页大小。
        Self {
            page: if self.page < 1 { 1 } else { self.page },
            page_size: if self.page_size < 1 {
                5
            } else {
                self.page_size
            },
            query: self.query,
            filter: self.filter,
            group_filter: self.group_filter,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountListResult {
    pub items: Vec<AccountSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAuthInfo {
    pub user_code_url: String,
    pub token_url: String,
    pub verification_url: String,
    pub redirect_uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStartResult {
    pub auth_url: String,
    pub login_id: String,
    pub login_type: String,
    pub issuer: String,
    pub client_id: String,
    pub redirect_uri: String,
    #[serde(default)]
    pub warning: Option<String>,
    pub device: Option<DeviceAuthInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshotResult {
    pub account_id: Option<String>,
    pub availability_status: Option<String>,
    pub used_percent: Option<f64>,
    pub window_minutes: Option<i64>,
    pub resets_at: Option<i64>,
    pub secondary_used_percent: Option<f64>,
    pub secondary_window_minutes: Option<i64>,
    pub secondary_resets_at: Option<i64>,
    pub credits_json: Option<String>,
    pub captured_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsageReadResult {
    pub snapshot: Option<UsageSnapshotResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitWindowResult {
    pub used_percent: i64,
    pub window_duration_mins: Option<i64>,
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitSnapshotResult {
    pub limit_id: Option<String>,
    pub limit_name: Option<String>,
    pub primary: Option<RateLimitWindowResult>,
    pub secondary: Option<RateLimitWindowResult>,
    pub credits: Option<serde_json::Value>,
    pub plan_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRateLimitsReadResult {
    pub rate_limits: RateLimitSnapshotResult,
    pub rate_limits_by_limit_id:
        Option<std::collections::BTreeMap<String, RateLimitSnapshotResult>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsageListResult {
    pub items: Vec<UsageSnapshotResult>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageAggregateSummaryResult {
    pub primary_bucket_count: i64,
    pub primary_known_count: i64,
    pub primary_unknown_count: i64,
    pub primary_remain_percent: Option<i64>,
    pub secondary_bucket_count: i64,
    pub secondary_known_count: i64,
    pub secondary_unknown_count: i64,
    pub secondary_remain_percent: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeySummary {
    pub id: String,
    pub name: Option<String>,
    pub model_slug: Option<String>,
    pub reasoning_effort: Option<String>,
    pub service_tier: Option<String>,
    pub rotation_strategy: String,
    pub aggregate_api_id: Option<String>,
    pub aggregate_api_url: Option<String>,
    pub client_type: String,
    pub protocol_type: String,
    pub auth_scheme: String,
    pub upstream_base_url: Option<String>,
    pub static_headers_json: Option<String>,
    pub status: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyListResult {
    pub items: Vec<ApiKeySummary>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyUsageStatSummary {
    pub key_id: String,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyUsageStatListResult {
    pub items: Vec<ApiKeyUsageStatSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyCreateResult {
    pub id: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeySecretResult {
    pub id: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateApiSummary {
    pub id: String,
    pub provider_type: String,
    pub supplier_name: Option<String>,
    pub sort: i64,
    pub url: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_test_at: Option<i64>,
    pub last_test_status: Option<String>,
    pub last_test_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AggregateApiListResult {
    pub items: Vec<AggregateApiSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateApiCreateResult {
    pub id: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateApiSecretResult {
    pub id: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateApiTestResult {
    pub id: String,
    pub ok: bool,
    pub status_code: Option<i64>,
    pub message: Option<String>,
    pub tested_at: i64,
    pub latency_ms: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOption {
    pub slug: String,
    pub display_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyModelListResult {
    pub items: Vec<ModelOption>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogSummary {
    pub trace_id: Option<String>,
    pub key_id: Option<String>,
    pub account_id: Option<String>,
    pub initial_account_id: Option<String>,
    #[serde(default)]
    pub attempted_account_ids: Vec<String>,
    pub initial_aggregate_api_id: Option<String>,
    #[serde(default)]
    pub attempted_aggregate_api_ids: Vec<String>,
    pub request_path: String,
    pub original_path: Option<String>,
    pub adapted_path: Option<String>,
    pub method: String,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub response_adapter: Option<String>,
    pub upstream_url: Option<String>,
    pub aggregate_api_supplier_name: Option<String>,
    pub aggregate_api_url: Option<String>,
    pub status_code: Option<i64>,
    pub duration_ms: Option<i64>,
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub reasoning_output_tokens: Option<i64>,
    pub estimated_cost_usd: Option<f64>,
    pub error: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RequestLogListParams {
    pub page: i64,
    pub page_size: i64,
    pub query: Option<String>,
    pub status_filter: Option<String>,
}

impl Default for RequestLogListParams {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 20,
            query: None,
            status_filter: None,
        }
    }
}

impl RequestLogListParams {
    pub fn normalized(self) -> Self {
        Self {
            page: if self.page < 1 { 1 } else { self.page },
            page_size: if self.page_size < 1 {
                20
            } else {
                self.page_size
            },
            query: self.query,
            status_filter: self.status_filter,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogListResult {
    pub items: Vec<RequestLogSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogFilterSummaryResult {
    pub total_count: i64,
    pub filtered_count: i64,
    pub success_count: i64,
    pub error_count: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogTodaySummaryResult {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub today_tokens: i64,
    pub estimated_cost: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupSnapshotResult {
    pub accounts: Vec<AccountSummary>,
    pub usage_snapshots: Vec<UsageSnapshotResult>,
    #[serde(default)]
    pub usage_aggregate_summary: UsageAggregateSummaryResult,
    pub api_keys: Vec<ApiKeySummary>,
    pub api_model_options: Vec<ModelOption>,
    pub manual_preferred_account_id: Option<String>,
    pub request_log_today_summary: RequestLogTodaySummaryResult,
    pub request_logs: Vec<RequestLogSummary>,
}

// ── Session management types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionProviderSummary {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListItem {
    pub id: String,
    pub file_path: String,
    pub relative_path: String,
    pub provider: String,
    pub source: String,
    pub timestamp: Option<String>,
    pub timestamp_display: String,
    pub cwd: Option<String>,
    pub originator: Option<String>,
    pub cli_version: Option<String>,
    pub preview: Option<String>,
    #[serde(default)]
    pub recent_prompts: Vec<String>,
    pub size: u64,
    pub size_display: String,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionListParams {
    pub page: i64,
    pub page_size: i64,
    pub query: Option<String>,
    pub provider: Option<String>,
    pub include_preview: bool,
}

impl Default for SessionListParams {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 20,
            query: None,
            provider: None,
            include_preview: false,
        }
    }
}

impl SessionListParams {
    pub fn normalized(self) -> Self {
        Self {
            page: if self.page < 1 { 1 } else { self.page },
            page_size: if self.page_size < 1 || self.page_size > 200 {
                20
            } else {
                self.page_size
            },
            query: self.query,
            provider: self.provider,
            include_preview: self.include_preview,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListTotals {
    pub all: i64,
    pub filtered: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListResult {
    #[serde(default)]
    pub items: Vec<SessionListItem>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub sessions_dir: String,
    #[serde(default)]
    pub providers: Vec<SessionProviderSummary>,
    pub totals: SessionListTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOverviewTotals {
    pub sessions: i64,
    pub providers: i64,
    pub backups: i64,
    pub bytes: u64,
    pub bytes_display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOverviewResult {
    pub sessions_dir: String,
    pub totals: SessionOverviewTotals,
    #[serde(default)]
    pub providers: Vec<SessionProviderSummary>,
    pub latest_session_at: Option<String>,
    pub latest_session_at_display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDashboardResult {
    pub overview: SessionOverviewResult,
    pub sessions: SessionListResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetailResult {
    pub id: String,
    pub file_path: String,
    pub relative_path: String,
    pub provider: String,
    pub source: String,
    pub timestamp: Option<String>,
    pub timestamp_display: String,
    pub cwd: Option<String>,
    pub originator: Option<String>,
    pub cli_version: Option<String>,
    pub size: u64,
    pub size_display: String,
    pub preview: Option<String>,
    #[serde(default)]
    pub recent_prompts: Vec<String>,
    pub latest_cwd: Option<String>,
    pub latest_model: Option<String>,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDoctorIssue {
    pub severity: String,
    pub issue_type: String,
    pub relative_path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionDoctorSummary {
    pub total_files: i64,
    pub invalid_meta_count: i64,
    pub missing_provider_count: i64,
    pub missing_workspace_count: i64,
    pub workspace_ready_count: i64,
    pub duplicate_id_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDoctorResult {
    pub ok: bool,
    pub sessions_dir: String,
    pub summary: SessionDoctorSummary,
    #[serde(default)]
    pub issues: Vec<SessionDoctorIssue>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionSelection {
    pub file_paths: Vec<String>,
    pub ids: Vec<String>,
    pub provider: Option<String>,
    pub query: Option<String>,
    pub limit: Option<i64>,
    pub allow_all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMigrationPreviewItem {
    pub id: String,
    pub file_path: String,
    pub relative_path: String,
    pub timestamp: Option<String>,
    pub timestamp_display: String,
    pub cwd: Option<String>,
    pub preview: Option<String>,
    pub from: String,
    pub from_source: String,
    pub to: String,
    pub to_source: String,
    pub skipped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMigrationPreviewResult {
    pub sessions_dir: String,
    pub target_provider: String,
    pub target_source: String,
    pub total_selected: i64,
    pub actionable: i64,
    pub skipped: i64,
    #[serde(default)]
    pub items: Vec<SessionMigrationPreviewItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionActionError {
    pub file_path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMigrationResult {
    pub ok: bool,
    pub dry_run: bool,
    pub sessions_dir: String,
    pub target_provider: String,
    pub target_source: String,
    pub backup_id: Option<String>,
    pub backup_dir: Option<String>,
    pub total_selected: i64,
    pub migrated: i64,
    pub skipped: i64,
    #[serde(default)]
    pub errors: Vec<SessionActionError>,
    #[serde(default)]
    pub items: Vec<SessionMigrationPreviewItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBackupSummary {
    pub backup_id: String,
    pub backup_dir: String,
    pub created_at: String,
    pub label: String,
    pub reason: Option<String>,
    pub source_provider: Option<String>,
    pub target_provider: Option<String>,
    pub entry_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBackupListResult {
    pub sessions_dir: String,
    #[serde(default)]
    pub backups: Vec<SessionBackupSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExportResult {
    pub ok: bool,
    pub sessions_dir: String,
    pub format: String,
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub content: String,
    pub session_count: i64,
    pub exported_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRepairResult {
    pub ok: bool,
    pub sessions_dir: String,
    pub session_index_path: String,
    pub session_index_backup_path: Option<String>,
    pub total_sessions: i64,
    pub written_entries: i64,
    pub state_database_count: i64,
    pub threads_inserted: i64,
    pub threads_updated: i64,
    #[serde(default)]
    pub issues: Vec<SessionDoctorIssue>,
}

#[cfg(test)]
#[path = "tests/types_tests.rs"]
mod tests;
