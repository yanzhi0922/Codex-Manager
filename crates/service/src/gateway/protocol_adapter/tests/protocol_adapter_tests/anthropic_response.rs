#[allow(unused_imports)]
use super::{adapt_request_for_protocol, adapt_upstream_response, ResponseAdapter};
use crate::apikey_profile::PROTOCOL_ANTHROPIC_NATIVE;

#[test]
fn anthropic_json_response_maps_reasoning_item_to_thinking_block() {
    let upstream = serde_json::json!({
        "id": "resp_reasoning_1",
        "object": "response",
        "created": 1700001200,
        "model": "gpt-5.3-codex",
        "output": [
            {
                "type": "reasoning",
                "id": "rs_1",
                "summary": [{"type": "summary_text", "text": "先检查配置，再执行。"}],
                "encrypted_content": "sig_reasoning_1"
            },
            {
                "type": "message",
                "content": [{"type": "output_text", "text": "已经处理完成。"}]
            }
        ],
        "usage": {"input_tokens": 12, "output_tokens": 6, "total_tokens": 18}
    });
    let (body, content_type) = adapt_upstream_response(
        ResponseAdapter::AnthropicJson,
        Some("application/json"),
        &serde_json::to_vec(&upstream).expect("serialize upstream"),
    )
    .expect("convert response");
    let value: serde_json::Value = serde_json::from_slice(&body).expect("parse converted body");
    assert_eq!(content_type, "application/json");
    assert_eq!(
        value
            .get("content")
            .and_then(|content| content.get(0))
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("thinking")
    );
    assert_eq!(
        value
            .get("content")
            .and_then(|content| content.get(0))
            .and_then(|block| block.get("thinking"))
            .and_then(serde_json::Value::as_str),
        Some("先检查配置，再执行。")
    );
    assert_eq!(
        value
            .get("content")
            .and_then(|content| content.get(0))
            .and_then(|block| block.get("signature"))
            .and_then(serde_json::Value::as_str),
        Some("sig_reasoning_1")
    );
}

#[test]
fn anthropic_sse_response_maps_reasoning_deltas_to_thinking_events() {
    let upstream = r#"data: {"type":"response.output_item.added","output_index":0,"item":{"type":"reasoning","id":"rs_stream_1","summary":[]}}

data: {"type":"response.reasoning_summary_text.delta","output_index":0,"summary_index":0,"delta":"先读配置"}

data: {"type":"response.output_item.done","output_index":0,"item":{"type":"reasoning","id":"rs_stream_1","summary":[{"type":"summary_text","text":"先读配置"}],"encrypted_content":"sig_stream_1"}}

data: [DONE]

"#;
    let (body, content_type) = adapt_upstream_response(
        ResponseAdapter::AnthropicSse,
        Some("text/event-stream"),
        upstream.as_bytes(),
    )
    .expect("convert response");
    let text = String::from_utf8(body).expect("parse sse body");
    assert_eq!(content_type, "text/event-stream");
    assert!(text.contains("\"type\":\"thinking_delta\""));
    assert!(text.contains("\"thinking\":\"先读配置\""));
    assert!(text.contains("\"type\":\"signature_delta\""));
    assert!(text.contains("\"signature\":\"sig_stream_1\""));
}

#[test]
fn anthropic_json_response_from_sse_preserves_thinking_block() {
    let upstream = r#"data: {"type":"response.output_item.added","output_index":0,"item":{"type":"reasoning","id":"rs_stream_json_1","summary":[]}}

data: {"type":"response.reasoning_text.delta","output_index":0,"content_index":0,"delta":"逐步分析"}

data: {"type":"response.output_item.done","output_index":0,"item":{"type":"reasoning","id":"rs_stream_json_1","summary":[],"encrypted_content":"sig_stream_json_1"}}

data: [DONE]

"#;
    let (body, content_type) = adapt_upstream_response(
        ResponseAdapter::AnthropicJson,
        Some("text/event-stream"),
        upstream.as_bytes(),
    )
    .expect("convert response");
    let value: serde_json::Value = serde_json::from_slice(&body).expect("parse converted body");
    assert_eq!(content_type, "application/json");
    assert_eq!(
        value
            .get("content")
            .and_then(|content| content.get(0))
            .and_then(|block| block.get("type"))
            .and_then(serde_json::Value::as_str),
        Some("thinking")
    );
    assert_eq!(
        value
            .get("content")
            .and_then(|content| content.get(0))
            .and_then(|block| block.get("thinking"))
            .and_then(serde_json::Value::as_str),
        Some("逐步分析")
    );
    assert_eq!(
        value
            .get("content")
            .and_then(|content| content.get(0))
            .and_then(|block| block.get("signature"))
            .and_then(serde_json::Value::as_str),
        Some("sig_stream_json_1")
    );
}

#[test]
fn anthropic_chat_completions_still_passthrough() {
    let body =
        br#"{"model":"gpt-5.3-codex","messages":[{"role":"user","content":"hello"}]}"#.to_vec();
    let adapted = adapt_request_for_protocol(
        PROTOCOL_ANTHROPIC_NATIVE,
        "/v1/chat/completions",
        body.clone(),
    )
    .expect("adapt request");
    assert_eq!(adapted.path, "/v1/chat/completions");
    assert_eq!(adapted.body, body);
    assert_eq!(adapted.response_adapter, ResponseAdapter::Passthrough);
}
