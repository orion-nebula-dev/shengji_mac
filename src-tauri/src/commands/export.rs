use std::{path::PathBuf, time::Instant};

use crate::{
    app::{export_service, runtime_observability_service},
    domain::export::{ExportBundleDto, GenerateExportBundleCommand},
    infra::sqlite::open_connection,
    AppState,
};

pub(crate) fn generate_export_bundle_payload(
    db_path: &PathBuf,
    command: GenerateExportBundleCommand,
) -> Result<ExportBundleDto, String> {
    let connection = open_connection(db_path)?;
    let started_at = Instant::now();
    let result = export_service::generate_export_bundle(&connection, command);
    let duration_ms = started_at.elapsed().as_millis() as i64;
    let status = if result.is_ok() {
        "succeeded"
    } else {
        "failed"
    };
    let error_message = result.as_ref().err().map(String::as_str).unwrap_or("");
    let _ = runtime_observability_service::record_runtime_metric(
        &connection,
        None,
        "generate_export_bundle",
        duration_ms,
        status,
        error_message,
    );
    result
}

#[tauri::command]
pub(crate) fn generate_export_bundle(
    command: GenerateExportBundleCommand,
    state: tauri::State<'_, AppState>,
) -> Result<ExportBundleDto, String> {
    generate_export_bundle_payload(&state.db_path, command)
}
