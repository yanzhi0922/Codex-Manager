use crate::commands::shared::rpc_call_in_background;

#[tauri::command]
pub async fn service_platforms_discovery(
    addr: Option<String>,
) -> Result<serde_json::Value, String> {
    rpc_call_in_background("platforms/discovery", addr, None).await
}
