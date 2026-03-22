mod db;
use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn check_db_status(client: tauri::State<'_, aws_sdk_dynamodb::Client>) -> Result<String, String> {
    Ok("Database client accessed successfully!".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Block the main thread just long enough to initialize the async DB client
            let db_client = tauri::async_runtime::block_on(async {
                db::init_client().await.expect("Failed to initialize database client")
            });

            // Pass the client into Tauri's state management
            app.manage(db_client);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet, check_db_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
