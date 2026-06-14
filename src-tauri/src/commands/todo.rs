use std::path::PathBuf;

use rusqlite::params;

use crate::{domain::todo::TodoDto, infra::sqlite::open_connection, AppState};

pub(crate) fn toggle_todo_status_payload(
    db_path: &PathBuf,
    todo_id: &str,
) -> Result<TodoDto, String> {
    let connection = open_connection(db_path)?;

    let current_status: String = connection
        .query_row(
            "SELECT status FROM todos WHERE id = ?1",
            params![todo_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取 Todo 状态失败: {error}"))?;

    let next_status = if current_status == "pending" {
        "completed"
    } else {
        "pending"
    };

    connection
        .execute(
            r#"
      UPDATE todos
      SET
        status = ?1,
        completed_at = CASE WHEN ?1 = 'completed' THEN CURRENT_TIMESTAMP ELSE NULL END,
        updated_at = CURRENT_TIMESTAMP
      WHERE id = ?2
      "#,
            params![next_status, todo_id],
        )
        .map_err(|error| format!("更新 Todo 状态失败: {error}"))?;

    connection
        .query_row(
            r#"
      SELECT
        id,
        title,
        note,
        status,
        created_at,
        conversation_session_id,
        IFNULL(source_text, '')
      FROM todos
      WHERE id = ?1
      "#,
            params![todo_id],
            |row| {
                Ok(TodoDto {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    note: row.get(2)?,
                    status: row.get(3)?,
                    created_at: row.get(4)?,
                    conversation_session_id: row.get(5)?,
                    source_text: row.get(6)?,
                })
            },
        )
        .map_err(|error| format!("读取更新后的 Todo 失败: {error}"))
}

#[tauri::command]
pub(crate) fn toggle_todo_status(
    todo_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<TodoDto, String> {
    toggle_todo_status_payload(&state.db_path, &todo_id)
}
