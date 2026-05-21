use super::*;

#[test]
fn login_complete_requires_params() {
    let req = JsonRpcRequest {
        id: 1,
        method: "account/login/complete".to_string(),
        params: None,
    };
    let resp = handle_request(req);
    let err = resp
        .result
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(err.contains("missing"));

    let req = JsonRpcRequest {
        id: 2,
        method: "account/login/complete".to_string(),
        params: Some(serde_json::json!({ "code": "x" })),
    };
    let resp = handle_request(req);
    let err = resp
        .result
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(err.contains("missing"));

    let req = JsonRpcRequest {
        id: 3,
        method: "account/login/complete".to_string(),
        params: Some(serde_json::json!({ "state": "y" })),
    };
    let resp = handle_request(req);
    let err = resp
        .result
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(err.contains("missing"));
}

#[test]
fn platforms_discovery_rpc_returns_readonly_matrix() {
    let req = JsonRpcRequest {
        id: 4,
        method: "platforms/discovery".to_string(),
        params: None,
    };
    let resp = handle_request(req);
    let items = resp
        .result
        .get("items")
        .and_then(|v| v.as_array())
        .expect("platform discovery items");

    assert!(
        items.iter().any(|item| {
            item.get("id")
                .and_then(|value| value.as_str())
                .map(|id| id == "codex")
                .unwrap_or(false)
        }),
        "missing Codex row: {}",
        resp.result
    );
    assert!(
        items.iter().any(|item| {
            item.get("id")
                .and_then(|value| value.as_str())
                .map(|id| id == "multi-instance")
                .unwrap_or(false)
        }),
        "missing multi-instance roadmap row: {}",
        resp.result
    );
    assert!(
        resp.result.get("totals").is_some(),
        "missing totals: {}",
        resp.result
    );
}
