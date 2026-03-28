mod models;
mod db;
use tauri::Manager;

use crate::{db::authenticate_user, db::register_user, models::error::AuthError};
use crate::{models::user::UserProfile};

#[tauri::command]
async fn login(
    db_client: tauri::State<'_, aws_sdk_dynamodb::Client>,
    username: String,
    password: String,
) -> Result<UserProfile, AuthError> {
    // We now return the UserProfile struct directly to the frontend. Tauri handles JSON serialization.
    match authenticate_user(&db_client, &username, &password).await {
        Ok(profile) => Ok(profile),
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
        Ok(uuid) => Ok(uuid), // Now returning the UUID so the frontend can immediately trigger a profile creation flow
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
            if let Some(window) = app.get_webview_window("main") {
                let logical_size = tauri::LogicalSize { width: 393.0, height: 852.0 };
                let _ = window.set_size(tauri::Size::Logical(logical_size));
                let _ = window.set_resizable(false); 
            }

            let db_client = tauri::async_runtime::block_on(async {
                let client = match db::init_client().await {
                    Ok(client) => client,
                    Err(e) => {
                        eprintln!("Critical error: Failed to initialize database client: {}", e);
                        std::process::exit(-1);
                    },
                };
                
                // Initialize both tables with their specific Partition Keys
                tracing::info!("Ensuring database tables exist...");
                db::create_table(&client, "users_auth", "username").await;
                db::create_table(&client, "users_profiles", "uuid").await;

                client
            });

            app.manage(db_client);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![login, register])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}