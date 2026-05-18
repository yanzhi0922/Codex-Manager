use codexmanager_core::rpc::types::{SessionExportResult, SessionListItem, SessionSelection};
use serde::Serialize;
use std::path::{Path, PathBuf};

use super::{jsonl_parser, prompt_utils, scanner};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptMessage {
    role: String,
    timestamp: String,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportSessionStats {
    message_count: usize,
    user_turns: usize,
    assistant_turns: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportSessionRecord {
    id: String,
    provider: String,
    relative_path: String,
    absolute_path: String,
    timestamp: String,
    timestamp_display: String,
    cwd: String,
    cli_version: String,
    originator: String,
    size: u64,
    size_display: String,
    preview: Option<String>,
    recent_prompts: Vec<String>,
    transcript: Vec<TranscriptMessage>,
    stats: ExportSessionStats,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportPayload {
    exported_at: String,
    exported_at_display: String,
    format: String,
    session_count: usize,
    sessions: Vec<ExportSessionRecord>,
}

fn validate_format(value: &str) -> Result<String, String> {
    let format = value.trim().to_ascii_lowercase();
    match format.as_str() {
        "markdown" | "md" => Ok("markdown".to_string()),
        "html" | "json" | "jsonl" | "csv" | "txt" => Ok(format),
        _ => Err(format!("unsupported export format: {format}")),
    }
}

fn normalize_text(text: &str) -> String {
    let normalized = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\0', "")
        .trim()
        .to_string();
    prompt_utils::sanitize_user_prompt(&normalized).unwrap_or_default()
}

fn extract_assistant_text(record: &serde_json::Value) -> Option<String> {
    let content = if record.get("type").and_then(|value| value.as_str()) == Some("response_item")
        && record
            .get("payload")
            .and_then(|payload| payload.get("role"))
            .and_then(|role| role.as_str())
            == Some("assistant")
    {
        record
            .get("payload")
            .and_then(|payload| payload.get("content"))
            .and_then(|content| content.as_array())
    } else if record.get("type").and_then(|value| value.as_str()) == Some("message")
        && record.get("role").and_then(|role| role.as_str()) == Some("assistant")
    {
        record.get("content").and_then(|content| content.as_array())
    } else {
        None
    }?;

    let mut parts = Vec::new();
    for item in content {
        let item_type = item.get("type").and_then(|value| value.as_str());
        if matches!(item_type, Some("output_text" | "text" | "markdown")) {
            if let Some(text) = item.get("text").and_then(|value| value.as_str()) {
                if !text.trim().is_empty() {
                    parts.push(text.to_string());
                }
            } else if let Some(text) = item
                .get("text")
                .and_then(|value| value.get("value"))
                .and_then(|value| value.as_str())
            {
                if !text.trim().is_empty() {
                    parts.push(text.to_string());
                }
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(normalize_text(&parts.join("\n\n")))
    }
}

fn dedupe_transcript(messages: Vec<TranscriptMessage>) -> Vec<TranscriptMessage> {
    let mut out: Vec<TranscriptMessage> = Vec::new();
    for message in messages {
        if out
            .last()
            .is_some_and(|prev| prev.role == message.role && prev.text == message.text)
        {
            continue;
        }
        out.push(message);
    }
    out
}

fn extract_transcript(file_path: &str) -> Vec<TranscriptMessage> {
    let content = match std::fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };
    let mut messages = Vec::new();

    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(record) => record,
            Err(_) => continue,
        };

        if let Some(text) = prompt_utils::extract_user_input_text(&record)
            .and_then(|text| prompt_utils::sanitize_user_prompt(&text))
        {
            messages.push(TranscriptMessage {
                role: "user".to_string(),
                timestamp: record
                    .get("timestamp")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                text,
            });
            continue;
        }

        if let Some(text) = extract_assistant_text(&record) {
            if !text.is_empty() {
                messages.push(TranscriptMessage {
                    role: "assistant".to_string(),
                    timestamp: record
                        .get("timestamp")
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_string(),
                    text,
                });
            }
        }
    }

    dedupe_transcript(messages)
}

fn build_record(item: &SessionListItem) -> ExportSessionRecord {
    let meta = Path::new(&item.file_path)
        .canonicalize()
        .ok()
        .and_then(|path| jsonl_parser::parse_first_line(&path).ok().flatten());
    let insights = jsonl_parser::extract_tail_insights(Path::new(&item.file_path));
    let transcript = extract_transcript(&item.file_path);
    let user_turns = transcript
        .iter()
        .filter(|entry| entry.role == "user")
        .count();
    let assistant_turns = transcript
        .iter()
        .filter(|entry| entry.role == "assistant")
        .count();

    ExportSessionRecord {
        id: item.id.clone(),
        provider: item.provider.clone(),
        relative_path: item.relative_path.clone(),
        absolute_path: item.file_path.clone(),
        timestamp: item
            .timestamp
            .clone()
            .or_else(|| meta.as_ref().and_then(|m| m.timestamp.clone()))
            .or(insights.latest_timestamp.clone())
            .unwrap_or_default(),
        timestamp_display: item.timestamp_display.clone(),
        cwd: item
            .cwd
            .clone()
            .or(insights.latest_cwd)
            .or_else(|| meta.as_ref().and_then(|m| m.cwd.clone()))
            .unwrap_or_default(),
        cli_version: item
            .cli_version
            .clone()
            .or_else(|| meta.as_ref().and_then(|m| m.cli_version.clone()))
            .unwrap_or_default(),
        originator: item
            .originator
            .clone()
            .or_else(|| meta.as_ref().and_then(|m| m.originator.clone()))
            .unwrap_or_default(),
        size: item.size,
        size_display: item.size_display.clone(),
        preview: item.preview.clone(),
        recent_prompts: item.recent_prompts.clone(),
        stats: ExportSessionStats {
            message_count: transcript.len(),
            user_turns,
            assistant_turns,
        },
        transcript,
    }
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch.is_whitespace() || ch == '-' || ch == '_') && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "export".to_string()
    } else {
        out
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn csv_escape(value: &str) -> String {
    if value.contains('"') || value.contains(',') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn render_markdown(payload: &ExportPayload) -> String {
    let mut lines = vec![
        "# Codex-Copilot Session Export".to_string(),
        String::new(),
        format!("- Generated at: {}", payload.exported_at_display),
        format!("- Format: {}", payload.format),
        format!("- Sessions: {}", payload.session_count),
        String::new(),
    ];

    for (index, session) in payload.sessions.iter().enumerate() {
        lines.push(format!("## {}. {}", index + 1, session.id));
        lines.push(String::new());
        lines.push(format!("- Provider: {}", session.provider));
        lines.push(format!(
            "- Time: {}",
            if session.timestamp_display.is_empty() {
                &session.timestamp
            } else {
                &session.timestamp_display
            }
        ));
        lines.push(format!("- Workspace: {}", session.cwd));
        lines.push(format!("- Path: {}", session.relative_path));
        lines.push(String::new());
        for message in &session.transcript {
            lines.push(format!("### {}", message.role));
            if !message.timestamp.is_empty() {
                lines.push(format!("_{}_", message.timestamp));
            }
            lines.push(String::new());
            lines.push(message.text.clone());
            lines.push(String::new());
        }
    }

    lines.join("\n")
}

fn render_text(payload: &ExportPayload) -> String {
    let mut lines = vec![
        "Codex-Copilot Session Export".to_string(),
        format!("Generated at: {}", payload.exported_at_display),
        format!("Sessions: {}", payload.session_count),
        String::new(),
    ];
    for session in &payload.sessions {
        lines.push(format!("== {} ==", session.id));
        lines.push(format!("Provider: {}", session.provider));
        lines.push(format!("Path: {}", session.relative_path));
        lines.push(String::new());
        for message in &session.transcript {
            lines.push(format!("[{}] {}", message.role, message.timestamp));
            lines.push(message.text.clone());
            lines.push(String::new());
        }
    }
    lines.join("\n")
}

fn render_csv(payload: &ExportPayload) -> String {
    let mut lines = vec!["session_id,provider,relative_path,role,timestamp,text".to_string()];
    for session in &payload.sessions {
        for message in &session.transcript {
            lines.push(format!(
                "{},{},{},{},{},{}",
                csv_escape(&session.id),
                csv_escape(&session.provider),
                csv_escape(&session.relative_path),
                csv_escape(&message.role),
                csv_escape(&message.timestamp),
                csv_escape(&message.text)
            ));
        }
    }
    lines.join("\n")
}

fn render_html(payload: &ExportPayload) -> String {
    let mut body = String::new();
    body.push_str("<!doctype html><html><head><meta charset=\"utf-8\"><title>Codex-Copilot Session Export</title>");
    body.push_str("<style>body{font-family:system-ui,-apple-system,Segoe UI,sans-serif;line-height:1.55;margin:32px;max-width:980px}.meta{color:#666}.msg{border-top:1px solid #ddd;padding:16px 0;white-space:pre-wrap}.role{font-weight:700}</style>");
    body.push_str("</head><body>");
    body.push_str("<h1>Codex-Copilot Session Export</h1>");
    body.push_str(&format!(
        "<p class=\"meta\">Generated at: {} · Sessions: {}</p>",
        escape_html(&payload.exported_at_display),
        payload.session_count
    ));
    for session in &payload.sessions {
        body.push_str(&format!(
            "<h2>{}</h2><p class=\"meta\">Provider: {} · Path: {}</p>",
            escape_html(&session.id),
            escape_html(&session.provider),
            escape_html(&session.relative_path)
        ));
        for message in &session.transcript {
            body.push_str(&format!(
                "<div class=\"msg\"><div class=\"role\">{} <span class=\"meta\">{}</span></div>{}</div>",
                escape_html(&message.role),
                escape_html(&message.timestamp),
                escape_html(&message.text)
            ));
        }
    }
    body.push_str("</body></html>");
    body
}

fn render_jsonl(payload: &ExportPayload) -> Result<String, String> {
    let mut lines = Vec::new();
    for session in &payload.sessions {
        lines.push(
            serde_json::to_string(session)
                .map_err(|err| format!("serialize export session: {err}"))?,
        );
    }
    Ok(lines.join("\n"))
}

fn render_payload(
    payload: &ExportPayload,
    format: &str,
) -> Result<(String, String, &'static str), String> {
    match format {
        "markdown" => Ok((
            render_markdown(payload),
            "md".to_string(),
            "text/markdown; charset=utf-8",
        )),
        "html" => Ok((
            render_html(payload),
            "html".to_string(),
            "text/html; charset=utf-8",
        )),
        "json" => Ok((
            serde_json::to_string_pretty(payload)
                .map_err(|err| format!("serialize export json: {err}"))?,
            "json".to_string(),
            "application/json; charset=utf-8",
        )),
        "jsonl" => Ok((
            render_jsonl(payload)?,
            "jsonl".to_string(),
            "application/x-ndjson; charset=utf-8",
        )),
        "csv" => Ok((
            render_csv(payload),
            "csv".to_string(),
            "text/csv; charset=utf-8",
        )),
        "txt" => Ok((
            render_text(payload),
            "txt".to_string(),
            "text/plain; charset=utf-8",
        )),
        _ => Err(format!("unsupported export format: {format}")),
    }
}

fn export_root(sessions_dir: &Path) -> PathBuf {
    sessions_dir
        .parent()
        .map(|parent| parent.join("exports"))
        .unwrap_or_else(|| sessions_dir.join("__exports__"))
}

pub(crate) fn export_sessions(
    sessions_dir: &Path,
    selection: &SessionSelection,
    format: &str,
    file_prefix: Option<&str>,
) -> Result<SessionExportResult, String> {
    let format = validate_format(format)?;
    let items = scanner::select_sessions(sessions_dir, selection, true)?;
    let sessions: Vec<ExportSessionRecord> = items.iter().map(build_record).collect();
    let exported_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let payload = ExportPayload {
        exported_at: exported_at.clone(),
        exported_at_display: jsonl_parser::format_timestamp_display(&exported_at),
        format: format.clone(),
        session_count: sessions.len(),
        sessions,
    };
    let (content, extension, mime_type) = render_payload(&payload, &format)?;

    let prefix = file_prefix
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("codex-copilot-session-export");
    let provider_suffix = {
        let mut providers: Vec<String> = payload
            .sessions
            .iter()
            .map(|session| session.provider.clone())
            .filter(|provider| !provider.is_empty())
            .collect();
        providers.sort();
        providers.dedup();
        if providers.len() == 1 {
            format!("-{}", slugify(&providers[0]))
        } else {
            String::new()
        }
    };
    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    let file_name = format!(
        "{}{}-{}x-{}.{}",
        slugify(prefix),
        provider_suffix,
        payload.session_count,
        timestamp,
        extension
    );
    let root = export_root(sessions_dir);
    std::fs::create_dir_all(&root).map_err(|err| format!("create {}: {err}", root.display()))?;
    let file_path = root.join(&file_name);
    std::fs::write(&file_path, &content)
        .map_err(|err| format!("write {}: {err}", file_path.display()))?;

    Ok(SessionExportResult {
        ok: true,
        sessions_dir: sessions_dir.to_string_lossy().to_string(),
        format,
        file_name,
        file_path: file_path.to_string_lossy().to_string(),
        mime_type: mime_type.to_string(),
        content,
        session_count: payload.session_count as i64,
        exported_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_sessions_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "codex-copilot-exporter-test-{:08x}",
            rand::random::<u32>()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn export_markdown_contains_transcript() {
        let dir = temp_sessions_dir();
        let file_path = dir.join("session.jsonl");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "session_meta",
                "payload": {
                    "id": "sess_export",
                    "model_provider": "openai",
                    "source": "vscode",
                    "timestamp": "2026-05-18T00:00:00Z",
                    "cwd": "C:\\work"
                }
            })
        )
        .unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "response_item",
                "timestamp": "2026-05-18T00:00:01Z",
                "payload": {
                    "role": "user",
                    "content": [{ "type": "input_text", "text": "summarize this" }]
                }
            })
        )
        .unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "response_item",
                "timestamp": "2026-05-18T00:00:02Z",
                "payload": {
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "done" }]
                }
            })
        )
        .unwrap();

        let result = export_sessions(
            &dir,
            &SessionSelection {
                allow_all: true,
                ..Default::default()
            },
            "markdown",
            None,
        )
        .unwrap();

        assert_eq!(result.session_count, 1);
        assert!(result.content.contains("summarize this"));
        assert!(result.content.contains("done"));
        assert!(Path::new(&result.file_path).is_file());

        let _ = std::fs::remove_file(&result.file_path);
        let _ = std::fs::remove_dir_all(dir);
    }
}
