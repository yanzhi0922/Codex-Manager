use codexmanager_core::rpc::types::{
    SessionBackupListResult, SessionBackupSummary, SessionListItem,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const BACKUP_ROOT_NAME: &str = "__backups__";
const FILES_DIR_NAME: &str = "files";
const MANIFEST_FILE_NAME: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifestEntry {
    id: Option<String>,
    relative_path: String,
    backup_relative_path: String,
    provider: Option<String>,
    timestamp: Option<String>,
    cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    backup_id: String,
    created_at: String,
    sessions_dir: String,
    label: String,
    reason: Option<String>,
    source_provider: Option<String>,
    target_provider: Option<String>,
    notes: Option<String>,
    entry_count: i64,
    entries: Vec<BackupManifestEntry>,
}

pub(crate) struct CreatedBackup {
    pub backup_id: String,
    pub backup_dir: PathBuf,
}

pub(crate) fn get_backup_root(sessions_dir: &Path) -> PathBuf {
    sessions_dir.join(BACKUP_ROOT_NAME)
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|err| format!("create {}: {err}", path.display()))
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
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "backup".to_string()
    } else {
        trimmed
    }
}

fn now_compact() -> String {
    chrono::Local::now().format("%Y%m%d%H%M%S").to_string()
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn create_backup_id(label: &str) -> String {
    let suffix = format!("{:06x}", rand::random::<u32>() & 0x00ff_ffff);
    format!("{}-{}-{}", now_compact(), slugify(label), suffix)
}

pub(crate) fn create_backup_snapshot(
    sessions_dir: &Path,
    entries: &[SessionListItem],
    label: &str,
    reason: Option<&str>,
    target_provider: Option<&str>,
) -> Result<CreatedBackup, String> {
    let backup_root = get_backup_root(sessions_dir);
    ensure_dir(&backup_root)?;

    let backup_id = create_backup_id(label);
    let backup_dir = backup_root.join(&backup_id);
    let files_dir = backup_dir.join(FILES_DIR_NAME);
    ensure_dir(&files_dir)?;

    let mut manifest_entries = Vec::with_capacity(entries.len());
    for entry in entries {
        let backup_relative_path = entry.relative_path.replace('\\', "/");
        let destination = files_dir.join(&backup_relative_path);
        if let Some(parent) = destination.parent() {
            ensure_dir(parent)?;
        }
        std::fs::copy(&entry.file_path, &destination).map_err(|err| {
            format!(
                "copy {} to {}: {err}",
                entry.file_path,
                destination.display()
            )
        })?;

        manifest_entries.push(BackupManifestEntry {
            id: if entry.id.is_empty() {
                None
            } else {
                Some(entry.id.clone())
            },
            relative_path: entry.relative_path.clone(),
            backup_relative_path,
            provider: if entry.provider.is_empty() {
                None
            } else {
                Some(entry.provider.clone())
            },
            timestamp: entry.timestamp.clone(),
            cwd: entry.cwd.clone(),
        });
    }

    let source_provider = entries
        .iter()
        .map(|entry| entry.provider.as_str())
        .find(|provider| !provider.is_empty())
        .map(str::to_string);

    let manifest = BackupManifest {
        backup_id: backup_id.clone(),
        created_at: now_iso(),
        sessions_dir: sessions_dir.to_string_lossy().to_string(),
        label: label.to_string(),
        reason: reason.map(str::to_string),
        source_provider,
        target_provider: target_provider.map(str::to_string),
        notes: None,
        entry_count: manifest_entries.len() as i64,
        entries: manifest_entries,
    };

    let manifest_path = backup_dir.join(MANIFEST_FILE_NAME);
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|err| format!("serialize backup manifest: {err}"))?;
    std::fs::write(&manifest_path, text)
        .map_err(|err| format!("write {}: {err}", manifest_path.display()))?;

    Ok(CreatedBackup {
        backup_id,
        backup_dir,
    })
}

pub(crate) fn list_backup_snapshots(
    sessions_dir: &Path,
) -> Result<SessionBackupListResult, String> {
    let backup_root = get_backup_root(sessions_dir);
    let mut backups = Vec::new();

    if backup_root.is_dir() {
        let entries = std::fs::read_dir(&backup_root)
            .map_err(|err| format!("read {}: {err}", backup_root.display()))?;
        for entry in entries.flatten() {
            let backup_dir = entry.path();
            if !backup_dir.is_dir() {
                continue;
            }
            let manifest_path = backup_dir.join(MANIFEST_FILE_NAME);
            let text = match std::fs::read_to_string(&manifest_path) {
                Ok(text) => text,
                Err(_) => continue,
            };
            let manifest: BackupManifest = match serde_json::from_str(&text) {
                Ok(manifest) => manifest,
                Err(_) => continue,
            };
            backups.push(SessionBackupSummary {
                backup_id: manifest.backup_id,
                backup_dir: backup_dir.to_string_lossy().to_string(),
                created_at: manifest.created_at,
                label: manifest.label,
                reason: manifest.reason,
                source_provider: manifest.source_provider,
                target_provider: manifest.target_provider,
                entry_count: manifest.entry_count,
            });
        }
    }

    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(SessionBackupListResult {
        sessions_dir: sessions_dir.to_string_lossy().to_string(),
        backups,
    })
}
