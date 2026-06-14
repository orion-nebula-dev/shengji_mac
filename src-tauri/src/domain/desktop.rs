use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopContext {
    pub(crate) runtime: String,
    pub(crate) platform: String,
    pub(crate) recorder_status: String,
    pub(crate) storage_status: String,
    pub(crate) models_status: String,
}
