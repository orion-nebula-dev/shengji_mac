use serde::Serialize;

use crate::domain::{runtime::RuntimeStatusDto, session::SessionDto};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecordingActionResult {
    pub(crate) message: String,
    pub(crate) runtime: RuntimeStatusDto,
    pub(crate) latest_session: Option<SessionDto>,
}
