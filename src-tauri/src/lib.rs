mod models;
mod db;
use tauri::Manager;

use crate::{db::authenticate_user, db::register_user, models::error::AuthError};

#[tauri::command]
async fn login(
    db_client: tauri::State<'_, aws_sdk_dynamodb::Client>,
    username: String,
    password: String,
) -> Result<String, AuthError> {
    match authenticate_user(&db_client, &username, &password).await {
        Ok(_) => Ok("Login successful!".to_string()),
        Err(e) => {
            match e {
                AuthError::Client(_) => Err(e),
                AuthError::Internal(msg) => {
                    tracing::debug!(msg);
                    Err(AuthError::Client("There was an error on the server side".to_string()))
                }
            }
        }
    }
}

#[tauri::command]
async fn register(
    db_client: tauri::State<'_, aws_sdk_dynamodb::Client>,
    username: String,
    password: String,
) -> Result<String, AuthError> {
    match register_user(&db_client, &username, &password).await {
        Ok(_) => Ok("Registration successful!".to_string()),
        Err(e) => {
            match e {
                AuthError::Client(_) => Err(e),
                AuthError::Internal(msg) => {
                    tracing::debug!(msg);
                    Err(AuthError::Client("There was an error on the server side".to_string()))
                }
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Resize window to mimic an iPhone 14/15 Pro screen
            if let Some(window) = app.get_webview_window("main") {
                let logical_size = tauri::LogicalSize { width: 393.0, height: 852.0 };
                let _ = window.set_size(tauri::Size::Logical(logical_size));
                let _ = window.set_resizable(false); // Lock it to prevent resizing
            }

            let db_client = tauri::async_runtime::block_on(async {
                let client = db::init_client().await.expect("Failed to initialize database client");
                
                let table_name = "test_table";
                tracing::info!("Ensuring table '{}' exists...", table_name);
                db::create_table(&client, table_name).await;

                // Removed hardcoded test_user registration

                client
            });

            app.manage(db_client);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![login, register])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}