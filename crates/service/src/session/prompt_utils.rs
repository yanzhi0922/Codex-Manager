use regex::Regex;
use std::sync::LazyLock;

static KEY_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"\bsk-[A-Za-z0-9_-]{16,}\b").unwrap(),
        Regex::new(r"\b(?:ghp|gho|ghu|github_pat)_[A-Za-z0-9_]{16,}\b").unwrap(),
        Regex::new(r"\bsk-ant-[A-Za-z0-9_-]{16,}\b").unwrap(),
        Regex::new(r"\bAIza[0-9A-Za-z_-]{20,}\b").unwrap(),
        Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap(),
        Regex::new(r"\bASIA[0-9A-Z]{16}\b").unwrap(),
        Regex::new(r"\bxox[baprs]-[A-Za-z0-9-]{16,}\b").unwrap(),
        Regex::new(r"\b(?:Bearer\s+)?eyJ[A-Za-z0-9._-]{20,}\b").unwrap(),
    ]
});

/// Extract user input text from a JSONL record.
pub fn extract_user_input_text(record: &serde_json::Value) -> Option<String> {
    let record_type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match record_type {
        "response_item" | "message" => {
            let payload = if record_type == "response_item" {
                record.get("payload")?
            } else {
                record
            };

            let role = payload.get("role").and_then(|v| v.as_str())?;
            if role != "user" {
                return None;
            }

            let content = payload.get("content")?.as_array()?;
            let parts: Vec<&str> = content
                .iter()
                .filter(|item| {
                    item.get("type")
                        .and_then(|v| v.as_str())
                        .map(|t| t == "input_text" || t == "text")
                        .unwrap_or(false)
                })
                .filter_map(|item| item.get("text").and_then(|v| v.as_str()))
                .collect();

            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        "compacted" => {
            let history = record
                .get("payload")
                .and_then(|p| p.get("replacement_history"))
                .and_then(|v| v.as_array())?;

            for entry in history.iter().rev() {
                if entry.get("type").and_then(|v| v.as_str()) != Some("message") {
                    continue;
                }
                if entry.get("role").and_then(|v| v.as_str()) != Some("user") {
                    continue;
                }
                if let Some(content) = entry.get("content").and_then(|v| v.as_array()) {
                    let parts: Vec<&str> = content
                        .iter()
                        .filter(|item| {
                            item.get("type")
                                .and_then(|v| v.as_str())
                                .map(|t| t == "input_text" || t == "text")
                                .unwrap_or(false)
                        })
                        .filter_map(|item| item.get("text").and_then(|v| v.as_str()))
                        .collect();
                    if !parts.is_empty() {
                        return Some(parts.join("\n"));
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Sanitize a user prompt: strip noise blocks and redact sensitive keys.
pub fn sanitize_user_prompt(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }

    let mut result = text.to_string();

    // Strip noise XML blocks.
    let noise_tags = [
        "environment_context",
        "turn_aborted",
        "INSTRUCTIONS",
        "app-context",
        "skills_instructions",
        "plugins_instructions",
        "collaboration_mode",
        "permissions instructions",
        "subagent_notification",
    ];
    for tag in &noise_tags {
        let pattern = format!("(?s)<{tag}[^>]*>.*?</{tag}>");
        if let Ok(re) = Regex::new(&pattern) {
            result = re.replace_all(&result, "").to_string();
        }
        // Also handle self-closing.
        let self_closing = format!("<{tag}[^>]*/>");
        if let Ok(re) = Regex::new(&self_closing) {
            result = re.replace_all(&result, "").to_string();
        }
    }

    // Strip AGENTS.md instructions headers.
    if let Ok(re) = Regex::new(r"(?m)^# AGENTS\.md instructions.*\n") {
        result = re.replace_all(&result, "").to_string();
    }
    if let Ok(re) = Regex::new(r"(?m)^# Developer Instructions.*\n") {
        result = re.replace_all(&result, "").to_string();
    }

    // Redact sensitive keys.
    for pattern in KEY_PATTERNS.iter() {
        result = pattern.replace_all(&result, "[redacted-key]").to_string();
    }

    let trimmed = result.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Summarize a prompt text to a max length with ellipsis.
pub fn summarize_prompt(text: &str, max_length: usize) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.len() <= max_length {
        return Some(trimmed.to_string());
    }
    // Find the last space before max_length to avoid cutting words.
    let end = trimmed[..max_length]
        .rfind(' ')
        .unwrap_or(max_length)
        .max(max_length.saturating_sub(20));
    Some(format!("{}...", &trimmed[..end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_user_input_text_response_item() {
        let record = serde_json::json!({
            "type": "response_item",
            "payload": {
                "role": "user",
                "content": [
                    { "type": "input_text", "text": "Hello world" }
                ]
            }
        });
        let result = extract_user_input_text(&record);
        assert_eq!(result, Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_user_input_text_wrong_role() {
        let record = serde_json::json!({
            "type": "response_item",
            "payload": {
                "role": "assistant",
                "content": [{ "type": "text", "text": "response" }]
            }
        });
        assert!(extract_user_input_text(&record).is_none());
    }

    #[test]
    fn test_sanitize_user_prompt_redacts_keys() {
        let text = "my key is sk-abc123def456ghi789jkl012mno345";
        let result = sanitize_user_prompt(text).unwrap();
        assert!(result.contains("[redacted-key]"));
        assert!(!result.contains("sk-abc123"));
    }

    #[test]
    fn test_sanitize_user_prompt_empty() {
        assert!(sanitize_user_prompt("").is_none());
        assert!(sanitize_user_prompt("   ").is_none());
    }

    #[test]
    fn test_summarize_prompt_short() {
        assert_eq!(summarize_prompt("hello", 10), Some("hello".to_string()));
    }

    #[test]
    fn test_summarize_prompt_long() {
        let text = "this is a long prompt that should be truncated at some reasonable point";
        let result = summarize_prompt(text, 30).unwrap();
        assert!(result.ends_with("..."));
        assert!(result.len() <= 33);
    }
}
