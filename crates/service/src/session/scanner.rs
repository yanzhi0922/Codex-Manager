use codexmanager_core::rpc::types::*;
use std::path::{Path, PathBuf};

use super::jsonl_parser::{self, format_size, format_timestamp_display};
use super::prompt_utils;
use super::tail_cache;

/// Directory names to skip when walking session files.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "__backup_duplicate_conflicts__",
    "sessions_backup",
];

/// Resolve the sessions directory from env or default.
pub fn get_sessions_dir(cli_dir: Option<&str>) -> Result<PathBuf, String> {
    if let Some(dir) = cli_dir {
        let path = PathBuf::from(dir);
        if path.is_dir() {
            return Ok(path);
        }
    }

    if let Ok(dir) = std::env::var("CODEX_SESSIONS_DIR") {
        let path = PathBuf::from(dir);
        if path.is_dir() {
            return Ok(path);
        }
    }

    // Default: ~/.codex/sessions
    let home = dirs_home()?;
    let codex_dir = home.join(".codex");
    let sessions_dir = codex_dir.join("sessions");

    if sessions_dir.is_dir() {
        Ok(sessions_dir)
    } else {
        Ok(codex_dir) // Return codex root; scanner will handle missing sessions subdir
    }
}

fn dirs_home() -> Result<PathBuf, String> {
    // Try HOME env var first (works on all platforms with MSYS2/Git Bash).
    if let Ok(home) = std::env::var("HOME") {
        let path = PathBuf::from(&home);
        if path.is_dir() {
            return Ok(path);
        }
    }
    // Try USERPROFILE (Windows native).
    if let Ok(home) = std::env::var("USERPROFILE") {
        let path = PathBuf::from(&home);
        if path.is_dir() {
            return Ok(path);
        }
    }
    Err("cannot determine home directory".to_string())
}

/// Check if a directory name should be skipped during walk.
fn should_skip_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name) || name.starts_with('.') || name.starts_with("__backup_")
}

/// Collect all .jsonl session files recursively.
pub(crate) fn walk_session_files(sessions_dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk_dir_recursive(sessions_dir, &mut files);
    files
}

fn walk_dir_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            if !should_skip_dir(&name_str) {
                walk_dir_recursive(&path, files);
            }
        } else if name_str.ends_with(".jsonl") {
            files.push(path);
        }
    }
}

/// Determine if a path is inside the archived_sessions directory.
fn is_archived_path(path: &Path, _sessions_dir: &Path) -> bool {
    path.to_string_lossy().contains("archived_sessions")
}

/// Build a SessionListItem from a file path.
fn build_session_item(
    file_path: &Path,
    sessions_dir: &Path,
    include_preview: bool,
) -> Option<SessionListItem> {
    let relative_path = file_path
        .strip_prefix(sessions_dir)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let file_size = file_path.metadata().ok().map(|m| m.len()).unwrap_or(0);
    let archived = is_archived_path(file_path, sessions_dir);

    // Try cache first.
    let meta = tail_cache::SESSION_META_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.get(file_path) {
            return Some(cached.clone());
        }
        None
    });

    let meta = match meta {
        Some(m) => m,
        None => {
            let parsed = jsonl_parser::parse_first_line(file_path).ok()??;
            tail_cache::SESSION_META_CACHE.with(|cache| {
                cache.borrow_mut().insert(file_path, parsed.clone());
            });
            parsed
        }
    };

    let timestamp_display = meta
        .timestamp
        .as_deref()
        .map(format_timestamp_display)
        .unwrap_or_default();

    let (preview, recent_prompts) = if include_preview {
        let insights = tail_cache::SESSION_TAIL_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if let Some(cached) = cache.get(file_path) {
                return Some(cached.clone());
            }
            None
        });

        let insights = match insights {
            Some(i) => i,
            None => {
                let i = jsonl_parser::extract_tail_insights(file_path);
                tail_cache::SESSION_TAIL_CACHE.with(|cache| {
                    cache.borrow_mut().insert(file_path, i.clone());
                });
                i
            }
        };

        let preview = insights
            .recent_prompts
            .first()
            .and_then(|p| prompt_utils::summarize_prompt(p, 100));

        (preview, insights.recent_prompts)
    } else {
        (None, Vec::new())
    };

    Some(SessionListItem {
        id: meta.id,
        file_path: file_path.to_string_lossy().to_string(),
        relative_path,
        provider: meta.model_provider,
        source: if meta.source.is_empty() {
            "unknown".to_string()
        } else {
            meta.source
        },
        timestamp: meta.timestamp,
        timestamp_display,
        cwd: meta.cwd,
        originator: meta.originator,
        cli_version: meta.cli_version,
        preview,
        recent_prompts,
        size: file_size,
        size_display: format_size(file_size),
        archived,
    })
}

/// Filter sessions by provider and search query.
fn filter_sessions(
    items: &[SessionListItem],
    provider: Option<&str>,
    query: Option<&str>,
) -> Vec<SessionListItem> {
    items
        .iter()
        .filter(|item| {
            if let Some(p) = provider {
                if !p.is_empty() && item.provider != p {
                    return false;
                }
            }
            if let Some(q) = query {
                if !q.is_empty() {
                    let lower_q = q.to_lowercase();
                    let searchable = format!(
                        "{} {} {} {} {}",
                        item.id,
                        item.provider,
                        item.relative_path,
                        item.cwd.as_deref().unwrap_or(""),
                        item.preview.as_deref().unwrap_or("")
                    )
                    .to_lowercase();
                    if !searchable.contains(&lower_q) {
                        return false;
                    }
                }
            }
            true
        })
        .cloned()
        .collect()
}

pub(crate) fn get_all_sessions(sessions_dir: &Path, include_preview: bool) -> Vec<SessionListItem> {
    let files = walk_session_files(sessions_dir);
    let mut items: Vec<SessionListItem> = files
        .iter()
        .filter_map(|f| build_session_item(f, sessions_dir, include_preview))
        .collect();

    items.sort_by(|a, b| {
        b.timestamp
            .cmp(&a.timestamp)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    items
}

pub(crate) fn select_sessions(
    sessions_dir: &Path,
    selection: &SessionSelection,
    include_preview: bool,
) -> Result<Vec<SessionListItem>, String> {
    let all_items = get_all_sessions(sessions_dir, include_preview);
    let has_explicit_selection = !selection.file_paths.is_empty()
        || !selection.ids.is_empty()
        || selection
            .provider
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty())
        || selection
            .query
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty());

    let mut selected = if !selection.file_paths.is_empty() {
        let allowed_paths: std::collections::HashSet<String> = selection
            .file_paths
            .iter()
            .map(|path| validate_session_path(path, sessions_dir))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect();
        all_items
            .into_iter()
            .filter(|item| allowed_paths.contains(&item.file_path))
            .collect()
    } else if !selection.ids.is_empty() {
        let ids: std::collections::HashSet<String> =
            selection.ids.iter().map(|id| id.to_string()).collect();
        all_items
            .into_iter()
            .filter(|item| ids.contains(&item.id))
            .collect()
    } else {
        if !has_explicit_selection && !selection.allow_all {
            return Err(
                "refusing to operate on the full session library without allowAll".to_string(),
            );
        }
        filter_sessions(
            &all_items,
            selection.provider.as_deref(),
            selection.query.as_deref(),
        )
    };

    if let Some(limit) = selection.limit {
        if limit > 0 {
            selected.truncate(limit as usize);
        }
    }

    Ok(selected)
}

/// Summarize providers from session items.
fn summarize_providers(items: &[SessionListItem]) -> Vec<SessionProviderSummary> {
    let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for item in items {
        *counts.entry(item.provider.clone()).or_insert(0) += 1;
    }

    let mut providers: Vec<SessionProviderSummary> = counts
        .into_iter()
        .map(|(name, count)| SessionProviderSummary { name, count })
        .collect();

    providers.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    providers
}

/// Count backup directories under __backups__.
fn count_backups(sessions_dir: &Path) -> i64 {
    let backups_dir = sessions_dir.join("__backups__");
    if !backups_dir.is_dir() {
        return 0;
    }
    std::fs::read_dir(&backups_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .count() as i64
        })
        .unwrap_or(0)
}

/// Scan sessions and return paginated, filtered results.
pub fn scan_sessions(
    sessions_dir: &Path,
    params: &SessionListParams,
) -> Result<SessionListResult, String> {
    let include_preview = params.include_preview || params.query.is_some();

    let items = get_all_sessions(sessions_dir, include_preview);

    let all_providers = summarize_providers(&items);
    let total_all = items.len() as i64;

    let filtered = filter_sessions(&items, params.provider.as_deref(), params.query.as_deref());
    let total_filtered = filtered.len() as i64;

    // Paginate.
    let page = params.page.max(1) as usize;
    let page_size = params.page_size.max(1) as usize;
    let start = (page - 1) * page_size;
    let page_items: Vec<SessionListItem> =
        filtered.into_iter().skip(start).take(page_size).collect();

    Ok(SessionListResult {
        items: page_items,
        total: total_filtered,
        page: params.page,
        page_size: params.page_size,
        sessions_dir: sessions_dir.to_string_lossy().to_string(),
        providers: all_providers,
        totals: SessionListTotals {
            all: total_all,
            filtered: total_filtered,
        },
    })
}

/// Get overview statistics for the sessions directory.
pub fn get_overview(sessions_dir: &Path) -> Result<SessionOverviewResult, String> {
    let files = walk_session_files(sessions_dir);

    let mut total_bytes: u64 = 0;
    let mut latest_ts: Option<String> = None;

    for file_path in &files {
        if let Ok(meta) = file_path.metadata() {
            total_bytes += meta.len();
        }
        if let Ok(Some(session_meta)) = jsonl_parser::parse_first_line(file_path) {
            if let Some(ref ts) = session_meta.timestamp {
                if latest_ts.as_ref().is_none_or(|l| ts > l) {
                    latest_ts = Some(ts.clone());
                }
            }
        }
    }

    let providers = {
        let items: Vec<SessionListItem> = files
            .iter()
            .filter_map(|f| build_session_item(f, sessions_dir, false))
            .collect();
        summarize_providers(&items)
    };

    let backup_count = count_backups(sessions_dir);

    Ok(SessionOverviewResult {
        sessions_dir: sessions_dir.to_string_lossy().to_string(),
        totals: SessionOverviewTotals {
            sessions: files.len() as i64,
            providers: providers.len() as i64,
            backups: backup_count,
            bytes: total_bytes,
            bytes_display: format_size(total_bytes),
        },
        providers,
        latest_session_at: latest_ts.clone(),
        latest_session_at_display: latest_ts
            .as_deref()
            .map(format_timestamp_display)
            .unwrap_or_default(),
    })
}

/// Get the session dashboard (overview + session list combined).
pub fn get_dashboard(
    sessions_dir: &Path,
    params: &SessionListParams,
) -> Result<SessionDashboardResult, String> {
    let overview = get_overview(sessions_dir)?;
    let sessions = scan_sessions(sessions_dir, params)?;
    Ok(SessionDashboardResult { overview, sessions })
}

/// Get detailed information about a single session.
pub fn get_session_detail(
    file_path_str: &str,
    sessions_dir: &Path,
) -> Result<SessionDetailResult, String> {
    let file_path = validate_session_path(file_path_str, sessions_dir)?;

    let meta = jsonl_parser::parse_first_line(&file_path)
        .map_err(|e| format!("parse session meta: {e}"))?
        .ok_or_else(|| "invalid session file: no session_meta found".to_string())?;

    let file_size = file_path.metadata().map(|m| m.len()).unwrap_or(0);
    let relative_path = file_path
        .strip_prefix(sessions_dir)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let archived = is_archived_path(&file_path, sessions_dir);

    let insights = jsonl_parser::extract_tail_insights(&file_path);
    let preview = insights
        .recent_prompts
        .first()
        .and_then(|p| prompt_utils::summarize_prompt(p, 200));

    Ok(SessionDetailResult {
        id: meta.id,
        file_path: file_path.to_string_lossy().to_string(),
        relative_path,
        provider: meta.model_provider,
        source: if meta.source.is_empty() {
            "unknown".to_string()
        } else {
            meta.source
        },
        timestamp: meta.timestamp.clone(),
        timestamp_display: meta
            .timestamp
            .as_deref()
            .map(format_timestamp_display)
            .unwrap_or_default(),
        cwd: meta.cwd,
        originator: meta.originator,
        cli_version: meta.cli_version,
        size: file_size,
        size_display: format_size(file_size),
        preview,
        recent_prompts: insights.recent_prompts,
        latest_cwd: insights.latest_cwd,
        latest_model: insights.latest_model,
        archived,
    })
}

/// Validate that a file path resolves inside the sessions directory (path traversal protection).
pub(crate) fn validate_session_path(
    file_path_str: &str,
    sessions_dir: &Path,
) -> Result<PathBuf, String> {
    let path = PathBuf::from(file_path_str);

    // Canonicalize both paths to resolve symlinks and '..' components.
    let canonical_path = path
        .canonicalize()
        .map_err(|e| format!("invalid file path: {e}"))?;

    // If the sessions_dir doesn't exist yet, just return the canonical path.
    if let Ok(canonical_dir) = sessions_dir.canonicalize() {
        if !canonical_path.starts_with(&canonical_dir) {
            return Err("file path is outside sessions directory".to_string());
        }
    }

    Ok(canonical_path)
}

/// Run doctor diagnostics on the sessions directory.
pub fn run_doctor(sessions_dir: &Path) -> Result<SessionDoctorResult, String> {
    let files = walk_session_files(sessions_dir);

    let mut summary = SessionDoctorSummary::default();
    let mut issues: Vec<SessionDoctorIssue> = Vec::new();
    let mut seen_ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    summary.total_files = files.len() as i64;

    for file_path in &files {
        let relative = file_path
            .strip_prefix(sessions_dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file_path.to_string_lossy().to_string());

        match jsonl_parser::parse_first_line(file_path) {
            Ok(None) => {
                summary.invalid_meta_count += 1;
                issues.push(SessionDoctorIssue {
                    severity: "error".to_string(),
                    issue_type: "invalid_meta".to_string(),
                    relative_path: Some(relative),
                    message: "first line is not a valid session_meta record".to_string(),
                });
                continue;
            }
            Err(e) => {
                summary.invalid_meta_count += 1;
                issues.push(SessionDoctorIssue {
                    severity: "error".to_string(),
                    issue_type: "invalid_meta".to_string(),
                    relative_path: Some(relative),
                    message: format!("parse error: {e}"),
                });
                continue;
            }
            Ok(Some(meta)) => {
                // Check for missing provider.
                if meta.model_provider.is_empty() {
                    summary.missing_provider_count += 1;
                    issues.push(SessionDoctorIssue {
                        severity: "warning".to_string(),
                        issue_type: "missing_provider".to_string(),
                        relative_path: Some(relative.clone()),
                        message: "no model_provider in session_meta".to_string(),
                    });
                }

                // Check for missing workspace.
                let cwd = meta.cwd.as_deref().unwrap_or("").trim();
                if cwd.is_empty() {
                    summary.missing_workspace_count += 1;
                    issues.push(SessionDoctorIssue {
                        severity: "warning".to_string(),
                        issue_type: "missing_workspace".to_string(),
                        relative_path: Some(relative.clone()),
                        message: "no workspace (cwd) in session_meta".to_string(),
                    });
                } else {
                    summary.workspace_ready_count += 1;
                }

                // Check for duplicate IDs.
                if !meta.id.is_empty() {
                    if let Some(first_path) = seen_ids.get(&meta.id) {
                        summary.duplicate_id_count += 1;
                        issues.push(SessionDoctorIssue {
                            severity: "warning".to_string(),
                            issue_type: "duplicate_id".to_string(),
                            relative_path: Some(relative),
                            message: format!("duplicate session ID (first seen at {first_path})"),
                        });
                    } else {
                        seen_ids.insert(meta.id, relative);
                    }
                }
            }
        }
    }

    if summary.workspace_ready_count == 0 && summary.total_files > 0 {
        summary.workspace_ready_count =
            summary.total_files - summary.invalid_meta_count - summary.missing_workspace_count;
    }

    let ok = summary.invalid_meta_count == 0
        && summary.duplicate_id_count == 0
        && issues.iter().all(|i| i.severity != "error");

    Ok(SessionDoctorResult {
        ok,
        sessions_dir: sessions_dir.to_string_lossy().to_string(),
        summary,
        issues,
    })
}
