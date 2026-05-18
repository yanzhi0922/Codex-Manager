use codexmanager_core::rpc::types::{
    SessionActionError, SessionListItem, SessionMigrationPreviewItem,
    SessionMigrationPreviewResult, SessionMigrationResult, SessionSelection,
};
use std::path::Path;

use super::{backup, scanner};

const DEFAULT_VISIBILITY_SOURCE: &str = "vscode";

fn normalize_provider(value: &str) -> Result<String, String> {
    let provider = value.trim();
    if provider.is_empty() {
        return Err("target provider is required".to_string());
    }
    if provider.contains('/') || provider.contains('\\') || provider.contains('\0') {
        return Err("target provider contains invalid path characters".to_string());
    }
    Ok(provider.to_string())
}

fn normalize_source(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn normalize_target_source(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(DEFAULT_VISIBILITY_SOURCE)
        .to_string()
}

fn is_subagent_source(value: &str) -> bool {
    value.starts_with('{') && value.contains("\"subagent\"")
}

fn should_normalize_source(current_source: &str, target_source: &str) -> bool {
    let source = normalize_source(Some(current_source));
    if target_source.is_empty() || source == target_source {
        return false;
    }
    is_subagent_source(&source) || matches!(source.as_str(), "unknown" | "exec" | "cli")
}

fn resolve_next_source(current_source: &str, target_source: &str, force_source: bool) -> String {
    if force_source {
        return target_source.to_string();
    }
    if should_normalize_source(current_source, target_source) {
        target_source.to_string()
    } else {
        current_source.to_string()
    }
}

fn needs_visibility_rewrite(
    item: &SessionListItem,
    target_provider: &str,
    target_source: &str,
) -> bool {
    item.provider != target_provider || should_normalize_source(&item.source, target_source)
}

fn build_preview_item(
    item: &SessionListItem,
    target_provider: &str,
    target_source: &str,
) -> SessionMigrationPreviewItem {
    let from_source = normalize_source(Some(&item.source));
    let to_source = normalize_source(Some(&resolve_next_source(
        &item.source,
        target_source,
        false,
    )));
    SessionMigrationPreviewItem {
        id: item.id.clone(),
        file_path: item.file_path.clone(),
        relative_path: item.relative_path.clone(),
        timestamp: item.timestamp.clone(),
        timestamp_display: item.timestamp_display.clone(),
        cwd: item.cwd.clone(),
        preview: item.preview.clone(),
        from: item.provider.clone(),
        from_source,
        to: target_provider.to_string(),
        to_source,
        skipped: !needs_visibility_rewrite(item, target_provider, target_source),
    }
}

fn build_preview(
    items: &[SessionListItem],
    target_provider: &str,
    target_source: &str,
) -> Vec<SessionMigrationPreviewItem> {
    items
        .iter()
        .map(|item| build_preview_item(item, target_provider, target_source))
        .collect()
}

fn split_first_line(content: &str) -> (&str, &str, &str) {
    if let Some(pos) = content.find('\n') {
        let mut first_line = &content[..pos];
        let newline = if first_line.ends_with('\r') {
            first_line = &first_line[..first_line.len() - 1];
            "\r\n"
        } else {
            "\n"
        };
        let remainder = &content[pos + 1..];
        (first_line, newline, remainder)
    } else {
        (content.trim_end_matches('\r'), "", "")
    }
}

fn rewrite_session_meta_in_file(
    file_path: &str,
    target_provider: &str,
    target_source: &str,
) -> Result<(), String> {
    let content =
        std::fs::read_to_string(file_path).map_err(|err| format!("read {file_path}: {err}"))?;
    let (first_line, newline, remainder) = split_first_line(&content);
    if first_line.trim().is_empty() {
        return Err("session file is empty".to_string());
    }

    let mut record: serde_json::Value =
        serde_json::from_str(first_line).map_err(|err| format!("first line json: {err}"))?;
    if record.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
        return Err("first line is not a session_meta record".to_string());
    }

    let payload = record
        .get_mut("payload")
        .and_then(|value| value.as_object_mut())
        .ok_or_else(|| "session_meta payload is not an object".to_string())?;
    let current_source = payload
        .get("source")
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string())
        })
        .unwrap_or_default();
    payload.insert(
        "model_provider".to_string(),
        serde_json::Value::String(target_provider.to_string()),
    );
    payload.insert(
        "source".to_string(),
        serde_json::Value::String(resolve_next_source(&current_source, target_source, false)),
    );

    let first_line =
        serde_json::to_string(&record).map_err(|err| format!("serialize session_meta: {err}"))?;
    let next_content = format!("{first_line}{newline}{remainder}");
    let temp_path = format!("{file_path}.tmp-{:08x}", rand::random::<u32>());

    std::fs::write(&temp_path, &next_content)
        .map_err(|err| format!("write temp {temp_path}: {err}"))?;
    match std::fs::rename(&temp_path, file_path) {
        Ok(_) => Ok(()),
        Err(rename_err) => {
            let fallback = std::fs::write(file_path, &next_content);
            let _ = std::fs::remove_file(&temp_path);
            fallback.map_err(|write_err| {
                format!("replace {file_path}: {rename_err}; fallback write failed: {write_err}")
            })
        }
    }
}

pub(crate) fn preview_migration(
    sessions_dir: &Path,
    selection: &SessionSelection,
    target_provider: &str,
    target_source: Option<&str>,
) -> Result<SessionMigrationPreviewResult, String> {
    let target_provider = normalize_provider(target_provider)?;
    let target_source = normalize_target_source(target_source);
    let items = scanner::select_sessions(sessions_dir, selection, true)?;
    let preview = build_preview(&items, &target_provider, &target_source);
    let actionable = preview.iter().filter(|item| !item.skipped).count() as i64;

    Ok(SessionMigrationPreviewResult {
        sessions_dir: sessions_dir.to_string_lossy().to_string(),
        target_provider,
        target_source,
        total_selected: items.len() as i64,
        actionable,
        skipped: preview.len() as i64 - actionable,
        items: preview,
    })
}

pub(crate) fn migrate_sessions(
    sessions_dir: &Path,
    selection: &SessionSelection,
    target_provider: &str,
    target_source: Option<&str>,
    dry_run: bool,
) -> Result<SessionMigrationResult, String> {
    let target_provider = normalize_provider(target_provider)?;
    let target_source = normalize_target_source(target_source);
    let items = scanner::select_sessions(sessions_dir, selection, true)?;
    let preview = build_preview(&items, &target_provider, &target_source);
    let actionable_items: Vec<SessionListItem> = items
        .into_iter()
        .filter(|item| needs_visibility_rewrite(item, &target_provider, &target_source))
        .collect();

    if dry_run {
        return Ok(SessionMigrationResult {
            ok: true,
            dry_run: true,
            sessions_dir: sessions_dir.to_string_lossy().to_string(),
            target_provider,
            target_source,
            backup_id: None,
            backup_dir: None,
            total_selected: preview.len() as i64,
            migrated: 0,
            skipped: preview.iter().filter(|item| item.skipped).count() as i64,
            errors: Vec::new(),
            items: preview,
        });
    }

    let backup = if actionable_items.is_empty() {
        None
    } else {
        Some(backup::create_backup_snapshot(
            sessions_dir,
            &actionable_items,
            "migration",
            Some("provider migration"),
            Some(&target_provider),
        )?)
    };

    let mut migrated = 0;
    let mut errors = Vec::new();
    for item in &actionable_items {
        match rewrite_session_meta_in_file(&item.file_path, &target_provider, &target_source) {
            Ok(_) => migrated += 1,
            Err(err) => errors.push(SessionActionError {
                file_path: item.file_path.clone(),
                message: err,
            }),
        }
    }

    Ok(SessionMigrationResult {
        ok: errors.is_empty(),
        dry_run: false,
        sessions_dir: sessions_dir.to_string_lossy().to_string(),
        target_provider,
        target_source,
        backup_id: backup.as_ref().map(|value| value.backup_id.clone()),
        backup_dir: backup
            .as_ref()
            .map(|value| value.backup_dir.to_string_lossy().to_string()),
        total_selected: preview.len() as i64,
        migrated,
        skipped: preview.iter().filter(|item| item.skipped).count() as i64,
        errors,
        items: preview,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_sessions_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "codex-copilot-migrator-test-{}-{:08x}",
            name,
            rand::random::<u32>()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_session(dir: &Path, provider: &str, source: &str) -> std::path::PathBuf {
        let file_path = dir.join("session.jsonl");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type": "session_meta",
                "payload": {
                    "id": "sess_1",
                    "model_provider": provider,
                    "source": source,
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
                    "content": [{ "type": "input_text", "text": "hello" }]
                }
            })
        )
        .unwrap();
        file_path
    }

    #[test]
    fn migrate_rewrites_session_meta_and_creates_backup() {
        let dir = temp_sessions_dir("rewrite");
        let file_path = write_session(&dir, "old", "cli");
        let result = migrate_sessions(
            &dir,
            &SessionSelection {
                allow_all: true,
                ..Default::default()
            },
            "new",
            Some("vscode"),
            false,
        )
        .unwrap();

        assert!(result.ok);
        assert_eq!(result.migrated, 1);
        assert!(result.backup_id.is_some());

        let content = std::fs::read_to_string(file_path).unwrap();
        let first_line = content.lines().next().unwrap();
        let record: serde_json::Value = serde_json::from_str(first_line).unwrap();
        assert_eq!(record["payload"]["model_provider"], "new");
        assert_eq!(record["payload"]["source"], "vscode");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn preview_marks_matching_sessions_as_skipped() {
        let dir = temp_sessions_dir("preview");
        write_session(&dir, "same", "vscode");
        let result = preview_migration(
            &dir,
            &SessionSelection {
                allow_all: true,
                ..Default::default()
            },
            "same",
            Some("vscode"),
        )
        .unwrap();

        assert_eq!(result.total_selected, 1);
        assert_eq!(result.actionable, 0);
        assert_eq!(result.skipped, 1);

        let _ = std::fs::remove_dir_all(dir);
    }
}
