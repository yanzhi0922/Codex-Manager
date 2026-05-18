use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// Maximum bytes to read when parsing the first JSONL line.
const MAX_FIRST_LINE_BYTES: usize = 256 * 1024;
/// Maximum bytes to read from the tail of a session file for insights.
const MAX_TAIL_INSIGHT_BYTES: usize = 2 * 1024 * 1024;

/// Parsed session metadata from the first JSONL line.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionMeta {
    pub id: String,
    pub model_provider: String,
    pub source: String,
    pub timestamp: Option<String>,
    pub cwd: Option<String>,
    pub originator: Option<String>,
    pub cli_version: Option<String>,
    pub agent_nickname: Option<String>,
    pub agent_role: Option<String>,
    pub agent_path: Option<String>,
}

/// Insights extracted from the tail of a session file.
#[derive(Debug, Clone, Default)]
pub struct TailInsights {
    pub latest_cwd: Option<String>,
    pub latest_model: Option<String>,
    pub latest_timestamp: Option<String>,
    pub recent_prompts: Vec<String>,
}

/// Result of reading a tail chunk from a file.
struct TailChunk {
    content: String,
    truncated_start: bool,
}

/// Open a file with retry on Windows file locking.
fn open_file_retry(path: &Path) -> std::io::Result<File> {
    let mut attempts = 0;
    loop {
        match File::open(path) {
            Ok(f) => return Ok(f),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && attempts < 3 => {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Parse the first line of a JSONL session file to extract session metadata.
pub fn parse_first_line(file_path: &Path) -> Result<Option<SessionMeta>, String> {
    let file = open_file_retry(file_path)
        .map_err(|e| format!("cannot open {}: {e}", file_path.display()))?;

    let mut reader = BufReader::new(file.take(MAX_FIRST_LINE_BYTES as u64));
    let mut head = String::new();
    reader
        .read_to_string(&mut head)
        .map_err(|e| format!("read head {}: {e}", file_path.display()))?;

    let first_line = match head.split('\n').next() {
        Some(line) => line.trim_end_matches('\r'),
        None => return Ok(None),
    };

    let record: serde_json::Value = match serde_json::from_str(first_line) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    if record.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
        return Ok(None);
    }

    let payload = match record.get("payload") {
        Some(p) if p.is_object() => p,
        _ => return Ok(None),
    };

    let id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let model_provider = payload
        .get("model_provider")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let source = payload
        .get("source")
        .map(|v| {
            if v.is_string() {
                v.as_str().unwrap_or("").to_string()
            } else {
                v.to_string()
            }
        })
        .unwrap_or_default();

    Ok(Some(SessionMeta {
        id,
        model_provider,
        source,
        timestamp: payload
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(String::from),
        cwd: payload
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(String::from),
        originator: payload
            .get("originator")
            .and_then(|v| v.as_str())
            .map(String::from),
        cli_version: payload
            .get("cli_version")
            .and_then(|v| v.as_str())
            .map(String::from),
        agent_nickname: payload
            .get("agent_nickname")
            .and_then(|v| v.as_str())
            .map(String::from),
        agent_role: payload
            .get("agent_role")
            .and_then(|v| v.as_str())
            .map(String::from),
        agent_path: payload
            .get("agent_path")
            .and_then(|v| v.as_str())
            .map(String::from),
    }))
}

/// Read the tail chunk of a file (up to `max_bytes` from the end).
fn read_tail_chunk(file_path: &Path, max_bytes: usize) -> Result<TailChunk, String> {
    let file = open_file_retry(file_path)
        .map_err(|e| format!("open tail {}: {e}", file_path.display()))?;

    let metadata = file
        .metadata()
        .map_err(|e| format!("stat {}: {e}", file_path.display()))?;
    let file_size = metadata.len() as usize;

    if file_size == 0 {
        return Ok(TailChunk {
            content: String::new(),
            truncated_start: false,
        });
    }

    let read_size = max_bytes.min(file_size);
    let offset = file_size - read_size;
    let truncated_start = offset > 0;

    let mut file = file;
    file.seek(SeekFrom::Start(offset as u64))
        .map_err(|e| format!("seek tail {}: {e}", file_path.display()))?;

    let mut buf = vec![0u8; read_size];
    file.read_exact(&mut buf)
        .map_err(|e| format!("read tail {}: {e}", file_path.display()))?;

    let content = String::from_utf8_lossy(&buf).into_owned();
    Ok(TailChunk {
        content,
        truncated_start,
    })
}

/// Extract insights from the tail of a session file.
pub fn extract_tail_insights(file_path: &Path) -> TailInsights {
    let chunk = match read_tail_chunk(file_path, MAX_TAIL_INSIGHT_BYTES) {
        Ok(c) => c,
        Err(_) => return TailInsights::default(),
    };

    let mut insights = TailInsights::default();
    let mut lines: Vec<&str> = chunk.content.split('\n').collect();

    // If truncated, discard the first partial line.
    if chunk.truncated_start && !lines.is_empty() {
        lines.remove(0);
    }

    // Scan lines in reverse for insights.
    for line in lines.iter().rev() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }

        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Extract latest cwd from turn_context.
        if record.get("type").and_then(|v| v.as_str()) == Some("turn_context") {
            if insights.latest_cwd.is_none() {
                if let Some(cwd) = record
                    .get("payload")
                    .and_then(|p| p.get("cwd"))
                    .and_then(|v| v.as_str())
                {
                    insights.latest_cwd = Some(cwd.to_string());
                }
            }
            if insights.latest_model.is_none() {
                if let Some(model) = record
                    .get("payload")
                    .and_then(|p| p.get("model"))
                    .and_then(|v| v.as_str())
                {
                    insights.latest_model = Some(model.to_string());
                }
            }
        }

        // Extract latest timestamp.
        if insights.latest_timestamp.is_none() {
            if let Some(ts) = record.get("timestamp").and_then(|v| v.as_str()) {
                insights.latest_timestamp = Some(ts.to_string());
            }
        }

        // Extract recent user prompts (up to 5).
        if insights.recent_prompts.len() < 5 {
            let role = record
                .get("payload")
                .and_then(|p| p.get("role"))
                .and_then(|v| v.as_str())
                .or_else(|| record.get("role").and_then(|v| v.as_str()));

            if role == Some("user") {
                if let Some(text) = super::prompt_utils::extract_user_input_text(&record) {
                    if let Some(sanitized) = super::prompt_utils::sanitize_user_prompt(&text) {
                        if !insights.recent_prompts.contains(&sanitized) {
                            insights.recent_prompts.push(sanitized);
                        }
                    }
                }
            }
        }

        // Early break if all fields collected.
        if insights.latest_cwd.is_some()
            && insights.latest_model.is_some()
            && insights.latest_timestamp.is_some()
            && insights.recent_prompts.len() >= 5
        {
            break;
        }
    }

    // Prompts are in reverse chronological order; reverse to get newest last.
    insights.recent_prompts.reverse();
    insights
}

/// Format a byte count as a human-readable string.
pub fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.1} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.1} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.1} KB", size as f64 / KB as f64)
    } else {
        format!("{size} B")
    }
}

/// Format an ISO timestamp for display (YYYY-MM-DD HH:MM).
pub fn format_timestamp_display(ts: &str) -> String {
    // Try to parse common ISO formats.
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.f") {
        return dt.format("%Y-%m-%d %H:%M").to_string();
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(ts, "%Y-%m-%d") {
        return d.format("%Y-%m-%d").to_string();
    }
    // Fallback: return as-is (truncated).
    if ts.len() >= 16 {
        ts[..16].replace('T', " ")
    } else {
        ts.to_string()
    }
}
