use std::path::PathBuf;

use crate::{
    app::export_service,
    domain::export::{ExportBundleDto, GenerateExportBundleCommand},
    infra::sqlite::open_connection,
    AppState,
};

pub(crate) fn generate_export_bundle_payload(
    db_path: &PathBuf,
    command: GenerateExportBundleCommand,
) -> Result<ExportBundleDto, String> {
    let connection = open_connection(db_path)?;
    export_service::generate_export_bundle(&connection, command)
}

#[tauri::command]
pub(crate) fn generate_export_bundle(
    command: GenerateExportBundleCommand,
    state: tauri::State<'_, AppState>,
) -> Result<ExportBundleDto, String> {
    generate_export_bundle_payload(&state.db_path, command)
}
