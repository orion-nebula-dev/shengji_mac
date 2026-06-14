use serde::Serialize;

use crate::domain::{runtime::RuntimeStatusDto, session::SessionDto, todo::TodoDto};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProcessingActionResult {
    pub(crate) message: String,
    pub(crate) runtime: RuntimeStatusDto,
    pub(crate) latest_session: Option<SessionDto>,
    pub(crate) todos: Vec<TodoDto>,
    pub(crate) sessions: Vec<SessionDto>,
}
