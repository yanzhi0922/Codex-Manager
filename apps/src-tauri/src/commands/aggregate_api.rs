use crate::commands::shared::rpc_call_in_background;

#[tauri::command]
pub async fn service_aggregate_api_list(
    addr: Option<String>,
) -> Result<serde_json::Value, String> {
    rpc_call_in_background("aggregateApi/list", addr, None).await
}

#[tauri::command]
pub async fn service_aggregate_api_create(
    addr: Option<String>,
    provider_type: Option<String>,
    supplier_name: Option<String>,
    sort: Option<i64>,
    url: Option<String>,
    key: Option<String>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({
        "providerType": provider_type,
        "supplierName": supplier_name,
        "sort": sort,
        "url": url,
        "key": key,
    });
    rpc_call_in_background("aggregateApi/create", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_aggregate_api_update(
    addr: Option<String>,
    api_id: String,
    provider_type: Option<String>,
    supplier_name: Option<String>,
    sort: Option<i64>,
    url: Option<String>,
    key: Option<String>,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({
        "id": api_id,
        "providerType": provider_type,
        "supplierName": supplier_name,
        "sort": sort,
        "url": url,
        "key": key,
    });
    rpc_call_in_background("aggregateApi/update", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_aggregate_api_read_secret(
    addr: Option<String>,
    api_id: String,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({ "id": api_id });
    rpc_call_in_background("aggregateApi/readSecret", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_aggregate_api_delete(
    addr: Option<String>,
    api_id: String,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({ "id": api_id });
    rpc_call_in_background("aggregateApi/delete", addr, Some(params)).await
}

#[tauri::command]
pub async fn service_aggregate_api_test_connection(
    addr: Option<String>,
    api_id: String,
) -> Result<serde_json::Value, String> {
    let params = serde_json::json!({ "id": api_id });
    rpc_call_in_background("aggregateApi/testConnection", addr, Some(params)).await
}
