use codexmanager_core::rpc::types::SessionListItem;
use codexmanager_core::rpc::types::{SessionDoctorIssue, SessionRepairResult};
use rusqlite::types::Value as SqlValue;
use rusqlite::{params_from_iter, Connection};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::scanner;

const SESSION_INDEX_FILENAME: &str = "session_index.jsonl";
const STATE_DB_PREFIX: &str = "state_";
const STATE_DB_SUFFIX: &str = ".sqlite";

fn codex_root(sessions_dir: &Path) -> PathBuf {
    sessions_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| sessions_dir.to_path_buf())
}

fn session_index_path(sessions_dir: &Path) -> PathBuf {
    codex_root(sessions_dir).join(SESSION_INDEX_FILENAME)
}

fn list_state_database_paths(sessions_dir: &Path) -> Vec<PathBuf> {
    let root = codex_root(sessions_dir);
    let mut paths = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return paths;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.is_file()
            && name.starts_with(STATE_DB_PREFIX)
            && name.to_ascii_lowercase().ends_with(STATE_DB_SUFFIX)
        {
            paths.push(path);
        }
    }
    paths.sort();
    paths
}

fn backup_existing_index(index_path: &Path) -> Result<Option<PathBuf>, String> {
    if !index_path.is_file() {
        return Ok(None);
    }
    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    let backup_path = index_path.with_extension(format!("jsonl.bak-{timestamp}"));
    std::fs::copy(index_path, &backup_path).map_err(|err| {
        format!(
            "backup {} to {}: {err}",
            index_path.display(),
            backup_path.display()
        )
    })?;
    Ok(Some(backup_path))
}

fn to_unix_seconds(value: Option<&str>) -> i64 {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return 0;
    };
    if let Ok(number) = value.parse::<f64>() {
        let mut normalized = number;
        while normalized > 1e11 {
            normalized /= 1000.0;
        }
        return normalized.max(0.0).floor() as i64;
    }
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|value| value.timestamp())
        .unwrap_or(0)
}

fn sqlite_has_threads_table(conn: &Connection) -> Result<bool, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'threads'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .map_err(|err| format!("query threads table: {err}"))
}

fn sqlite_thread_columns(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(threads)")
        .map_err(|err| format!("prepare table_info: {err}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|err| format!("query table_info: {err}"))?;
    let mut columns = Vec::new();
    for row in rows {
        columns.push(row.map_err(|err| format!("read table_info: {err}"))?);
    }
    Ok(columns)
}

fn sql_value_text(value: impl Into<String>) -> SqlValue {
    SqlValue::Text(value.into())
}

fn thread_column_value(item: &SessionListItem, column: &str) -> Option<SqlValue> {
    let created_at = to_unix_seconds(item.timestamp.as_deref());
    let updated_at = created_at.max(0);
    let title = item
        .preview
        .clone()
        .or_else(|| item.recent_prompts.first().cloned())
        .or_else(|| {
            item.cwd
                .as_deref()
                .and_then(|cwd| Path::new(cwd).file_name())
                .map(|name| name.to_string_lossy().to_string())
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| item.id.clone());
    let first_user_message = item
        .recent_prompts
        .first()
        .cloned()
        .unwrap_or_else(|| title.clone());

    match column {
        "id" => Some(sql_value_text(item.id.clone())),
        "rollout_path" => Some(sql_value_text(item.file_path.clone())),
        "created_at" => Some(SqlValue::Integer(created_at)),
        "updated_at" => Some(SqlValue::Integer(updated_at)),
        "source" => Some(sql_value_text(item.source.clone())),
        "model_provider" => Some(sql_value_text(item.provider.clone())),
        "cwd" => Some(sql_value_text(item.cwd.clone().unwrap_or_default())),
        "title" => Some(sql_value_text(title)),
        "sandbox_policy" => Some(sql_value_text(r#"{"type":"unknown"}"#)),
        "approval_mode" => Some(sql_value_text("unknown")),
        "tokens_used" => Some(SqlValue::Integer(0)),
        "has_user_event" => Some(SqlValue::Integer((!item.recent_prompts.is_empty()) as i64)),
        "archived" => Some(SqlValue::Integer(item.archived as i64)),
        "archived_at" => {
            if item.archived {
                Some(SqlValue::Integer(updated_at))
            } else {
                Some(SqlValue::Null)
            }
        }
        "cli_version" => Some(sql_value_text(item.cli_version.clone().unwrap_or_default())),
        "first_user_message" => Some(sql_value_text(first_user_message)),
        "model" => Some(SqlValue::Null),
        "reasoning_effort" => Some(SqlValue::Null),
        "agent_nickname" => Some(SqlValue::Null),
        "agent_role" => Some(SqlValue::Null),
        "agent_path" => Some(SqlValue::Null),
        "memory_mode" => Some(sql_value_text("enabled")),
        "git_sha" => Some(SqlValue::Null),
        "git_branch" => Some(SqlValue::Null),
        "git_origin_url" => Some(SqlValue::Null),
        _ => None,
    }
}

fn existing_thread_count(conn: &Connection, item: &SessionListItem) -> Result<i64, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM threads WHERE id = ?1 OR rollout_path = ?2",
        [&item.id, &item.file_path],
        |row| row.get::<_, i64>(0),
    )
    .map_err(|err| format!("query existing thread {}: {err}", item.id))
}

fn update_thread_row(
    conn: &Connection,
    columns: &[String],
    item: &SessionListItem,
) -> Result<usize, String> {
    let update_columns: Vec<&String> = columns
        .iter()
        .filter(|column| {
            !matches!(
                column.as_str(),
                "id" | "rollout_path" | "created_at" | "tokens_used"
            ) && thread_column_value(item, column).is_some()
        })
        .collect();
    if update_columns.is_empty() {
        return Ok(0);
    }
    let assignments = update_columns
        .iter()
        .map(|column| format!("{column} = ?"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("UPDATE threads SET {assignments} WHERE id = ? OR rollout_path = ?");
    let mut values: Vec<SqlValue> = update_columns
        .iter()
        .filter_map(|column| thread_column_value(item, column))
        .collect();
    values.push(sql_value_text(item.id.clone()));
    values.push(sql_value_text(item.file_path.clone()));
    conn.execute(&sql, params_from_iter(values))
        .map_err(|err| format!("update thread {}: {err}", item.id))
}

fn insert_thread_row(
    conn: &Connection,
    columns: &[String],
    item: &SessionListItem,
) -> Result<usize, String> {
    let insert_columns: Vec<&String> = columns
        .iter()
        .filter(|column| thread_column_value(item, column).is_some())
        .collect();
    if insert_columns.is_empty() {
        return Ok(0);
    }
    let names = insert_columns
        .iter()
        .map(|column| column.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = vec!["?"; insert_columns.len()].join(", ");
    let sql = format!("INSERT INTO threads ({names}) VALUES ({placeholders})");
    let values: Vec<SqlValue> = insert_columns
        .iter()
        .filter_map(|column| thread_column_value(item, column))
        .collect();
    conn.execute(&sql, params_from_iter(values))
        .map_err(|err| format!("insert thread {}: {err}", item.id))
}

fn sync_threads_table(db_path: &Path, items: &[SessionListItem]) -> Result<(i64, i64), String> {
    let conn = Connection::open(db_path)
        .map_err(|err| format!("open state database {}: {err}", db_path.display()))?;
    conn.busy_timeout(Duration::from_millis(1500))
        .map_err(|err| format!("set busy timeout {}: {err}", db_path.display()))?;
    if !sqlite_has_threads_table(&conn)? {
        return Ok((0, 0));
    }
    let columns = sqlite_thread_columns(&conn)?;
    if !columns.iter().any(|column| column == "id")
        || !columns.iter().any(|column| column == "rollout_path")
    {
        return Err("threads table does not expose id and rollout_path columns".to_string());
    }

    let mut inserted = 0;
    let mut updated = 0;
    for item in items {
        if item.id.trim().is_empty() {
            continue;
        }
        if existing_thread_count(&conn, item)? > 0 {
            updated += update_thread_row(&conn, &columns, item)? as i64;
        } else {
            inserted += insert_thread_row(&conn, &columns, item)? as i64;
        }
    }

    Ok((inserted, updated))
}

pub(crate) fn repair_session_index(sessions_dir: &Path) -> Result<SessionRepairResult, String> {
    let items = scanner::get_all_sessions(sessions_dir, true);
    let index_path = session_index_path(sessions_dir);
    if let Some(parent) = index_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    let backup_path = backup_existing_index(&index_path)?;

    let mut issues = Vec::new();
    let mut lines = Vec::new();
    for item in &items {
        if item.id.trim().is_empty() {
            issues.push(SessionDoctorIssue {
                severity: "warning".to_string(),
                issue_type: "missing_id".to_string(),
                relative_path: Some(item.relative_path.clone()),
                message: "session_meta has no id; skipped from session_index".to_string(),
            });
            continue;
        }

        let timestamp = item
            .timestamp
            .clone()
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let updated_at = timestamp.clone();
        let title = item
            .preview
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                item.cwd
                    .as_deref()
                    .and_then(|cwd| {
                        Path::new(cwd)
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                    })
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| item.id.clone())
            });

        let record = json!({
            "id": item.id,
            "rollout_path": item.file_path,
            "created_at": timestamp,
            "updated_at": updated_at,
            "source": item.source,
            "model_provider": item.provider,
            "cwd": item.cwd,
            "title": title,
            "cli_version": item.cli_version,
            "has_user_event": !item.recent_prompts.is_empty(),
            "archived": item.archived,
            "archived_at": if item.archived { item.timestamp.clone() } else { None::<String> },
        });
        lines.push(
            serde_json::to_string(&record)
                .map_err(|err| format!("serialize session_index row: {err}"))?,
        );
    }

    let content = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };
    let temp_path = index_path.with_extension(format!("jsonl.tmp-{:08x}", rand::random::<u32>()));
    std::fs::write(&temp_path, content)
        .map_err(|err| format!("write temp {}: {err}", temp_path.display()))?;
    match std::fs::rename(&temp_path, &index_path) {
        Ok(_) => {}
        Err(rename_err) => {
            let fallback = std::fs::write(&index_path, lines.join("\n"));
            let _ = std::fs::remove_file(&temp_path);
            fallback.map_err(|write_err| {
                format!(
                    "replace {}: {rename_err}; fallback write failed: {write_err}",
                    index_path.display()
                )
            })?;
        }
    }

    let state_database_paths = list_state_database_paths(sessions_dir);
    let mut threads_inserted = 0;
    let mut threads_updated = 0;
    for db_path in &state_database_paths {
        match sync_threads_table(db_path, &items) {
            Ok((inserted, updated)) => {
                threads_inserted += inserted;
                threads_updated += updated;
            }
            Err(err) => issues.push(SessionDoctorIssue {
                severity: "warning".to_string(),
                issue_type: "threads_sync_failed".to_string(),
                relative_path: db_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_string),
                message: err,
            }),
        }
    }

    Ok(SessionRepairResult {
        ok: issues.iter().all(|issue| issue.severity != "error"),
        sessions_dir: sessions_dir.to_string_lossy().to_string(),
        session_index_path: index_path.to_string_lossy().to_string(),
        session_index_backup_path: backup_path.map(|path| path.to_string_lossy().to_string()),
        total_sessions: items.len() as i64,
        written_entries: lines.len() as i64,
        state_database_count: state_database_paths.len() as i64,
        threads_inserted,
        threads_updated,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_codex_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "codex-copilot-repair-test-{:08x}",
            rand::random::<u32>()
        ));
        std::fs::create_dir_all(root.join("sessions")).unwrap();
        root
    }

    fn write_session(sessions_dir: &Path) {
        let mut file = std::fs::File::create(sessions_dir.join("session.jsonl")).unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "session_meta",
                "payload": {
                    "id": "sess_repair",
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
                "payload": {
                    "role": "user",
                    "content": [{ "type": "input_text", "text": "repair this" }]
                }
            })
        )
        .unwrap();
    }

    #[test]
    fn repair_writes_session_index_and_threads_table() {
        let root = temp_codex_root();
        let sessions_dir = root.join("sessions");
        write_session(&sessions_dir);

        let db_path = root.join("state_test.sqlite");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT,
                created_at INTEGER,
                updated_at INTEGER,
                source TEXT,
                model_provider TEXT,
                cwd TEXT,
                title TEXT,
                has_user_event INTEGER,
                archived INTEGER
            )",
            [],
        )
        .unwrap();
        drop(conn);

        let result = repair_session_index(&sessions_dir).unwrap();
        assert_eq!(result.written_entries, 1);
        assert_eq!(result.state_database_count, 1);
        assert_eq!(result.threads_inserted, 1);
        assert!(Path::new(&result.session_index_path).is_file());

        let conn = Connection::open(db_path).unwrap();
        let provider: String = conn
            .query_row(
                "SELECT model_provider FROM threads WHERE id = 'sess_repair'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(provider, "openai");

        let _ = std::fs::remove_dir_all(root);
    }
}
