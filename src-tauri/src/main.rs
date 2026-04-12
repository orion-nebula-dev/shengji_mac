// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::var("SMART_TODO_PROCESS_PENDING_ONCE")
        .ok()
        .as_deref()
        == Some("1")
    {
        let db_path = std::env::var("SMART_TODO_DB_PATH").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/Library/Application Support/com.smarttodo.desktop/smart-todo.sqlite")
        });

        match app_lib::process_pending_jobs_once_for_cli(&db_path) {
            Ok(message) => {
                println!("{message}");
                return;
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }

    if std::env::var("SMART_TODO_EMBEDDED_TODO_RUNTIME")
        .ok()
        .as_deref()
        == Some("1")
    {
        match app_lib::run_embedded_todo_runtime_once() {
            Ok(()) => return,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }

    app_lib::run();
}
