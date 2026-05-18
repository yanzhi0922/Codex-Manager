use codexmanager_core::rpc::types::{
    JsonRpcRequest, JsonRpcResponse, SessionListParams, SessionSelection,
};

use crate::session::{backup, exporter, migrator, repair, scanner};

fn selection_param(req: &JsonRpcRequest) -> Result<SessionSelection, String> {
    req.params
        .as_ref()
        .and_then(|value| value.get("selection"))
        .cloned()
        .map(serde_json::from_value::<SessionSelection>)
        .transpose()
        .map_err(|err| format!("invalid session selection: {err}"))
        .map(|selection| selection.unwrap_or_default())
}

pub(super) fn try_handle(req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    let result = match req.method.as_str() {
        "session/scan" => {
            let params = req
                .params
                .clone()
                .map(serde_json::from_value::<SessionListParams>)
                .transpose()
                .map(|params| params.unwrap_or_default())
                .map(SessionListParams::normalized)
                .map_err(|err| format!("invalid session/scan params: {err}"));
            super::value_or_error(params.and_then(|p| {
                let dir = super::string_param(req, "sessionsDir");
                let sessions_dir = scanner::get_sessions_dir(dir.as_deref())?;
                scanner::scan_sessions(&sessions_dir, &p)
            }))
        }
        "session/overview" => {
            let dir = super::string_param(req, "sessionsDir");
            let result =
                scanner::get_sessions_dir(dir.as_deref()).and_then(|d| scanner::get_overview(&d));
            super::value_or_error(result)
        }
        "session/detail" => {
            let path = super::string_param(req, "path").unwrap_or_default();
            let dir = super::string_param(req, "sessionsDir");
            let result = scanner::get_sessions_dir(dir.as_deref())
                .and_then(|d| scanner::get_session_detail(&path, &d));
            super::value_or_error(result)
        }
        "session/dashboard" => {
            let params = req
                .params
                .clone()
                .map(serde_json::from_value::<SessionListParams>)
                .transpose()
                .map(|params| params.unwrap_or_default())
                .map(SessionListParams::normalized)
                .map_err(|err| format!("invalid session/dashboard params: {err}"));
            super::value_or_error(params.and_then(|p| {
                let dir = super::string_param(req, "sessionsDir");
                let sessions_dir = scanner::get_sessions_dir(dir.as_deref())?;
                scanner::get_dashboard(&sessions_dir, &p)
            }))
        }
        "session/doctor" => {
            let dir = super::string_param(req, "sessionsDir");
            let result =
                scanner::get_sessions_dir(dir.as_deref()).and_then(|d| scanner::run_doctor(&d));
            super::value_or_error(result)
        }
        "session/migratePreview" => {
            let result = selection_param(req).and_then(|selection| {
                let dir = super::string_param(req, "sessionsDir");
                let target_provider = super::string_param(req, "targetProvider")
                    .ok_or_else(|| "targetProvider is required".to_string())?;
                let target_source = super::string_param(req, "targetSource");
                let sessions_dir = scanner::get_sessions_dir(dir.as_deref())?;
                migrator::preview_migration(
                    &sessions_dir,
                    &selection,
                    &target_provider,
                    target_source.as_deref(),
                )
            });
            super::value_or_error(result)
        }
        "session/migrate" => {
            let result = selection_param(req).and_then(|selection| {
                let dir = super::string_param(req, "sessionsDir");
                let target_provider = super::string_param(req, "targetProvider")
                    .ok_or_else(|| "targetProvider is required".to_string())?;
                let target_source = super::string_param(req, "targetSource");
                let dry_run = super::bool_param(req, "dryRun").unwrap_or(false);
                let sessions_dir = scanner::get_sessions_dir(dir.as_deref())?;
                migrator::migrate_sessions(
                    &sessions_dir,
                    &selection,
                    &target_provider,
                    target_source.as_deref(),
                    dry_run,
                )
            });
            super::value_or_error(result)
        }
        "session/export" => {
            let result = selection_param(req).and_then(|selection| {
                let dir = super::string_param(req, "sessionsDir");
                let format =
                    super::string_param(req, "format").unwrap_or_else(|| "markdown".to_string());
                let file_prefix = super::string_param(req, "filePrefix");
                let sessions_dir = scanner::get_sessions_dir(dir.as_deref())?;
                exporter::export_sessions(
                    &sessions_dir,
                    &selection,
                    &format,
                    file_prefix.as_deref(),
                )
            });
            super::value_or_error(result)
        }
        "session/repair" => {
            let dir = super::string_param(req, "sessionsDir");
            let result = scanner::get_sessions_dir(dir.as_deref())
                .and_then(|d| repair::repair_session_index(&d));
            super::value_or_error(result)
        }
        "session/backups" => {
            let dir = super::string_param(req, "sessionsDir");
            let result = scanner::get_sessions_dir(dir.as_deref())
                .and_then(|d| backup::list_backup_snapshots(&d));
            super::value_or_error(result)
        }
        _ => return None,
    };

    Some(super::response(req, result))
}
