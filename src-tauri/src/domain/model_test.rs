use serde::{Deserialize, Serialize};

use crate::domain::settings::SettingsDto;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelTestRequest {
    pub(crate) provider: String,
    pub(crate) settings: SettingsDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelTestResult {
    pub(crate) provider: String,
    pub(crate) success: bool,
    pub(crate) status_code: u16,
    pub(crate) message: String,
    pub(crate) response_excerpt: String,
}
