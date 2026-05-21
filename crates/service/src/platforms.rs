use codexmanager_core::rpc::types::{
    PlatformDiscoveryItem, PlatformDiscoveryResult, PlatformDiscoveryTotals,
};
use codexmanager_core::storage::now_ts;
use std::path::PathBuf;

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn home_dir() -> Option<PathBuf> {
    env_path("USERPROFILE").or_else(|| env_path("HOME"))
}

fn app_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env_path("APPDATA")
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|home| home.join("Library").join("Application Support"))
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        env_path("XDG_CONFIG_HOME").or_else(|| home_dir().map(|home| home.join(".config")))
    }
}

fn existing_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().filter(|path| path.exists()).collect()
}

fn display_path(path: &std::path::Path) -> String {
    path.to_string_lossy().to_string()
}

fn status_for_paths(detected: &[PathBuf], planned_only: bool) -> String {
    if planned_only {
        "planned".to_string()
    } else if detected.is_empty() {
        "missing".to_string()
    } else {
        "detected".to_string()
    }
}

fn build_item(
    id: &str,
    name: &str,
    category: &str,
    candidates: Vec<PathBuf>,
    notes: Vec<&str>,
) -> PlatformDiscoveryItem {
    let detected = existing_paths(candidates);
    PlatformDiscoveryItem {
        id: id.to_string(),
        name: name.to_string(),
        category: category.to_string(),
        status: status_for_paths(&detected, false),
        primary_path: detected.first().map(|path| display_path(path)),
        detected_paths: detected.iter().map(|path| display_path(path)).collect(),
        signals: detected
            .iter()
            .map(|path| format!("path:{}", display_path(path)))
            .collect(),
        notes: notes.into_iter().map(str::to_string).collect(),
    }
}

fn codex_item() -> PlatformDiscoveryItem {
    let codex_home = env_path("CODEX_HOME")
        .or_else(|| home_dir().map(|home| home.join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"));
    let sessions_dir = codex_home.join("sessions");
    let mut signals = Vec::new();
    let mut detected_paths = Vec::new();

    if codex_home.exists() {
        signals.push("codex_home".to_string());
        detected_paths.push(display_path(&codex_home));
    }
    if sessions_dir.exists() {
        signals.push("sessions".to_string());
        detected_paths.push(display_path(&sessions_dir));
    }

    let status = if sessions_dir.exists() {
        "ready"
    } else if codex_home.exists() {
        "detected"
    } else {
        "missing"
    };

    PlatformDiscoveryItem {
        id: "codex".to_string(),
        name: "Codex".to_string(),
        category: "core".to_string(),
        status: status.to_string(),
        primary_path: detected_paths.first().cloned(),
        detected_paths,
        signals,
        notes: vec!["Codex-Copilot 的核心平台；账号池、网关、会话管理已经围绕它闭环。".to_string()],
    }
}

fn vscode_like_candidates(folder: &str) -> Vec<PathBuf> {
    app_data_dir()
        .map(|base| vec![base.join(folder).join("User"), base.join(folder)])
        .unwrap_or_default()
}

fn home_candidates(folder: &str) -> Vec<PathBuf> {
    home_dir()
        .map(|home| vec![home.join(folder)])
        .unwrap_or_default()
}

fn planned_item(id: &str, name: &str, category: &str, notes: Vec<&str>) -> PlatformDiscoveryItem {
    PlatformDiscoveryItem {
        id: id.to_string(),
        name: name.to_string(),
        category: category.to_string(),
        status: "planned".to_string(),
        primary_path: None,
        detected_paths: Vec::new(),
        signals: Vec::new(),
        notes: notes.into_iter().map(str::to_string).collect(),
    }
}

fn build_platform_items() -> Vec<PlatformDiscoveryItem> {
    let mut items = vec![
        codex_item(),
        build_item(
            "vscode",
            "VS Code / GitHub Copilot",
            "ai-ide",
            vscode_like_candidates("Code"),
            vec!["只读探测 VS Code 用户目录，后续可承载 Copilot 实例边界。"],
        ),
        build_item(
            "cursor",
            "Cursor",
            "ai-ide",
            vscode_like_candidates("Cursor"),
            vec!["只读探测 Cursor 用户目录；写入凭证前必须先设计备份和预览。"],
        ),
        build_item(
            "windsurf",
            "Windsurf",
            "ai-ide",
            vscode_like_candidates("Windsurf"),
            vec!["适合复用实例目录与启动参数模型，当前仅做存在性探测。"],
        ),
        build_item(
            "kiro",
            "Kiro",
            "ai-ide",
            vscode_like_candidates("Kiro"),
            vec!["先记录本地目录是否存在，再评估账号注入风险。"],
        ),
        build_item(
            "gemini-cli",
            "Gemini CLI",
            "cli",
            home_candidates(".gemini"),
            vec!["只读探测 ~/.gemini；适合后续做导入/导出和状态显示。"],
        ),
        build_item(
            "zed",
            "Zed",
            "ai-ide",
            vscode_like_candidates("Zed")
                .into_iter()
                .chain(home_candidates(".config/zed"))
                .collect(),
            vec!["Zed 各平台路径差异较大，当前仅做低风险目录探测。"],
        ),
    ];

    items.push(planned_item(
        "multi-instance",
        "Codex 多实例工作区",
        "roadmap",
        vec!["下一阶段目标：实例目录、启动命令、绑定账号和生命周期管理。"],
    ));
    items
}

fn summarize_totals(items: &[PlatformDiscoveryItem]) -> PlatformDiscoveryTotals {
    let mut totals = PlatformDiscoveryTotals::default();
    for item in items {
        match item.status.as_str() {
            "ready" => totals.ready += 1,
            "detected" => totals.detected += 1,
            "planned" => totals.planned += 1,
            _ => totals.missing += 1,
        }
    }
    totals
}

pub(crate) fn discover_platforms() -> PlatformDiscoveryResult {
    let items = build_platform_items();
    let totals = summarize_totals(&items);
    PlatformDiscoveryResult {
        generated_at: now_ts(),
        totals,
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_includes_codex_and_planned_instance_row() {
        let result = discover_platforms();
        assert!(result.items.iter().any(|item| item.id == "codex"));
        assert!(result.items.iter().any(|item| item.id == "multi-instance"));
        assert_eq!(
            result.totals.ready
                + result.totals.detected
                + result.totals.missing
                + result.totals.planned,
            result.items.len() as i64
        );
    }
}
