use super::*;
use codexmanager_core::storage::AggregateApi;

#[test]
fn account_rotation_priority_falls_back_to_aggregate_api() {
    let _lock = lock_env();
    let dir = new_test_dir("codexmanager-gateway-account-priority-fallback-aggregate");
    let db_path: PathBuf = dir.join("codexmanager.db");
    let _db_guard = EnvGuard::set("CODEXMANAGER_DB_PATH", db_path.to_string_lossy().as_ref());

    let aggregate_response = serde_json::json!({
        "id": "resp_account_priority_fallback",
        "model": "gpt-5.3-codex",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "output_text", "text": "aggregate ok" }]
        }],
        "usage": { "input_tokens": 6, "output_tokens": 3, "total_tokens": 9 }
    });
    let aggregate_response =
        serde_json::to_string(&aggregate_response).expect("serialize aggregate response");
    let (aggregate_addr, aggregate_rx, aggregate_join) =
        start_mock_upstream_once(&aggregate_response);

    let storage = Storage::open(&db_path).expect("open db");
    storage.init().expect("init db");
    let now = now_ts();
    storage
        .insert_aggregate_api(&AggregateApi {
            id: "agg_account_priority_fallback".to_string(),
            provider_type: "codex".to_string(),
            supplier_name: Some("aggregate-test".to_string()),
            sort: 0,
            url: format!("http://{aggregate_addr}"),
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
            last_test_at: None,
            last_test_status: None,
            last_test_error: None,
        })
        .expect("insert aggregate api");
    storage
        .upsert_aggregate_api_secret("agg_account_priority_fallback", "aggregate_secret")
        .expect("insert aggregate secret");

    let platform_key = "pk_account_priority_fallback";
    storage
        .insert_api_key(&ApiKey {
            id: "gk_account_priority_fallback".to_string(),
            name: Some("account-priority".to_string()),
            model_slug: Some("gpt-5.3-codex".to_string()),
            reasoning_effort: Some("high".to_string()),
            service_tier: None,
            rotation_strategy: "account_rotation".to_string(),
            aggregate_api_id: None,
            aggregate_api_url: None,
            client_type: "codex".to_string(),
            protocol_type: "openai_compat".to_string(),
            auth_scheme: "authorization_bearer".to_string(),
            upstream_base_url: None,
            static_headers_json: None,
            key_hash: hash_platform_key_for_test(platform_key),
            status: "active".to_string(),
            created_at: now,
            last_used_at: None,
        })
        .expect("insert api key");

    let server = codexmanager_service::start_one_shot_server().expect("start server");
    let request_body = serde_json::json!({
        "model": "gpt-5.3-codex",
        "input": "hello",
        "stream": false
    });
    let request_body = serde_json::to_string(&request_body).expect("serialize request");
    let (status, gateway_body) = post_http_raw(
        &server.addr,
        "/v1/responses",
        &request_body,
        &[
            ("Content-Type", "application/json"),
            ("Authorization", &format!("Bearer {platform_key}")),
        ],
    );
    server.join();
    assert_eq!(status, 200, "gateway response: {gateway_body}");

    let aggregate_captured = aggregate_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("receive aggregate request");
    aggregate_join.join().expect("join aggregate upstream");
    assert_eq!(aggregate_captured.path, "/v1/responses");
    assert_eq!(
        aggregate_captured
            .headers
            .get("authorization")
            .map(String::as_str),
        Some("Bearer aggregate_secret")
    );
}

#[test]
fn aggregate_api_rotation_priority_falls_back_to_account() {
    let _lock = lock_env();
    let dir = new_test_dir("codexmanager-gateway-aggregate-priority-fallback-account");
    let db_path: PathBuf = dir.join("codexmanager.db");
    let _db_guard = EnvGuard::set("CODEXMANAGER_DB_PATH", db_path.to_string_lossy().as_ref());

    let upstream_response = serde_json::json!({
        "id": "resp_aggregate_priority_fallback",
        "model": "gpt-5.3-codex",
        "output": [{
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "output_text", "text": "account ok" }]
        }],
        "usage": { "input_tokens": 8, "output_tokens": 4, "total_tokens": 12 }
    });
    let upstream_response =
        serde_json::to_string(&upstream_response).expect("serialize upstream response");
    let (upstream_addr, upstream_rx, upstream_join) = start_mock_upstream_once(&upstream_response);
    let upstream_base = format!("http://{upstream_addr}/backend-api/codex");
    let _upstream_guard = EnvGuard::set("CODEXMANAGER_UPSTREAM_BASE_URL", &upstream_base);

    let storage = Storage::open(&db_path).expect("open db");
    storage.init().expect("init db");
    let now = now_ts();
    storage
        .insert_account(&Account {
            id: "acc_aggregate_priority_fallback".to_string(),
            label: "aggregate-priority-fallback".to_string(),
            issuer: "https://auth.openai.com".to_string(),
            chatgpt_account_id: Some("chatgpt_aggregate_priority_fallback".to_string()),
            workspace_id: None,
            group_name: None,
            sort: 0,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        })
        .expect("insert account");
    storage
        .insert_token(&Token {
            account_id: "acc_aggregate_priority_fallback".to_string(),
            id_token: String::new(),
            access_token: "access_token_aggregate_priority_fallback".to_string(),
            refresh_token: String::new(),
            api_key_access_token: Some("api_access_token_aggregate_priority_fallback".to_string()),
            last_refresh: now,
        })
        .expect("insert token");

    let platform_key = "pk_aggregate_priority_fallback";
    storage
        .insert_api_key(&ApiKey {
            id: "gk_aggregate_priority_fallback".to_string(),
            name: Some("aggregate-priority".to_string()),
            model_slug: Some("gpt-5.3-codex".to_string()),
            reasoning_effort: Some("high".to_string()),
            service_tier: None,
            rotation_strategy: "aggregate_api_rotation".to_string(),
            aggregate_api_id: None,
            aggregate_api_url: None,
            client_type: "codex".to_string(),
            protocol_type: "openai_compat".to_string(),
            auth_scheme: "authorization_bearer".to_string(),
            upstream_base_url: None,
            static_headers_json: None,
            key_hash: hash_platform_key_for_test(platform_key),
            status: "active".to_string(),
            created_at: now,
            last_used_at: None,
        })
        .expect("insert api key");

    let server = codexmanager_service::start_one_shot_server().expect("start server");
    let request_body = serde_json::json!({
        "model": "gpt-5.3-codex",
        "input": "hello",
        "stream": false
    });
    let request_body = serde_json::to_string(&request_body).expect("serialize request");
    let (status, gateway_body) = post_http_raw(
        &server.addr,
        "/v1/responses",
        &request_body,
        &[
            ("Content-Type", "application/json"),
            ("Authorization", &format!("Bearer {platform_key}")),
        ],
    );
    server.join();
    assert_eq!(status, 200, "gateway response: {gateway_body}");

    let captured = upstream_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("receive upstream request");
    upstream_join.join().expect("join upstream");
    assert_eq!(captured.path, "/backend-api/codex/responses");
    assert_eq!(
        captured.headers.get("authorization").map(String::as_str),
        Some("Bearer access_token_aggregate_priority_fallback")
    );
}
