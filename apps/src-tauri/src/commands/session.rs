use crate::commands::shared::rpc_call_in_background;

#[tauri::command]
pub async fn service_session_scan(
    addr: Option<String>,
    sessions_dir: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
    query: Option<String>,
    provider: Option<String>,
    include_preview: Option<bool>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({
        "sessionsDir": sessions_dir,
        "page": page,
        "pageSize": page_size,
        "query": query,
        "provider": provider,
        "includePreview": include_preview
    });
    rpc_call_in_background("session/scan", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_session_overview(
    addr: Option<String>,
    sessions_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({ "sessionsDir": sessions_dir });
    rpc_call_in_background("session/overview", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_session_detail(
    addr: Option<String>,
    path: Option<String>,
    sessions_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({
        "path": path,
        "sessionsDir": sessions_dir
    });
    rpc_call_in_background("session/detail", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_session_dashboard(
    addr: Option<String>,
    sessions_dir: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
    query: Option<String>,
    provider: Option<String>,
    include_preview: Option<bool>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({
        "sessionsDir": sessions_dir,
        "page": page,
        "pageSize": page_size,
        "query": query,
        "provider": provider,
        "includePreview": include_preview
    });
    rpc_call_in_background("session/dashboard", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_session_doctor(
    addr: Option<String>,
    sessions_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({ "sessionsDir": sessions_dir });
    rpc_call_in_background("session/doctor", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_session_migrate_preview(
    addr: Option<String>,
    sessions_dir: Option<String>,
    selection: Option<serde_json::Value>,
    target_provider: Option<String>,
    target_source: Option<String>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({
        "sessionsDir": sessions_dir,
        "selection": selection.unwrap_or_else(|| serde_json::json!({})),
        "targetProvider": target_provider,
        "targetSource": target_source
    });
    rpc_call_in_background("session/migratePreview", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_session_migrate(
    addr: Option<String>,
    sessions_dir: Option<String>,
    selection: Option<serde_json::Value>,
    target_provider: Option<String>,
    target_source: Option<String>,
    dry_run: Option<bool>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({
        "sessionsDir": sessions_dir,
        "selection": selection.unwrap_or_else(|| serde_json::json!({})),
        "targetProvider": target_provider,
        "targetSource": target_source,
        "dryRun": dry_run
    });
    rpc_call_in_background("session/migrate", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_session_export(
    addr: Option<String>,
    sessions_dir: Option<String>,
    selection: Option<serde_json::Value>,
    format: Option<String>,
    file_prefix: Option<String>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({
        "sessionsDir": sessions_dir,
        "selection": selection.unwrap_or_else(|| serde_json::json!({})),
        "format": format,
        "filePrefix": file_prefix
    });
    rpc_call_in_background("session/export", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_session_repair(
    addr: Option<String>,
    sessions_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({ "sessionsDir": sessions_dir });
    rpc_call_in_background("session/repair", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_session_backups(
    addr: Option<String>,
    sessions_dir: Option<String>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({ "sessionsDir": sessions_dir });
    rpc_call_in_background("session/backups", addr, Some(params)).await
}
