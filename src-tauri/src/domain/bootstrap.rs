use serde::Serialize;

use crate::domain::{
    runtime::RuntimeStatusDto, session::SessionDto, settings::SettingsDto, todo::TodoDto,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BootstrapData {
    pub(crate) settings: SettingsDto,
    pub(crate) todos: Vec<TodoDto>,
    pub(crate) sessions: Vec<SessionDto>,
    pub(crate) runtime: RuntimeStatusDto,
}
