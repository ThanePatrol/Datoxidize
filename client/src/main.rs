mod sync_logic;
mod http_sync;

use std::path::Path;
use notify::*;
use crate::sync_logic::{create_folder_on_remote, deserialize_config, sync_changed_file};
use notify::event::CreateKind::Folder;
use notify::EventKind::Create;

#[tokio::main]
async fn main() -> Result<()> {


    let dir_settings = deserialize_config("./client/test_resources/config.json".to_string()).unwrap();
    let watched_dir = &dir_settings.content_directory.clone();
    let frequency = dir_settings.sync_frequency.clone();

    http_sync::init_sync().await;

    //sync_logic::initial_sync(&dir_settings);

    //todo - create a atomic boolean value to limit writes in short succession
    //todo - use a timer to determine when file was last modified
    //todo - if there is another event before this timer reaches 0, reset the timer
    //todo - so if a file is edited many times it will only be synced after a period of inactivity

    //NB - This watcher needs to be initiated like this to allow for asynchronous runtime be
    //between web server and notifications
    let mut watcher =
        RecommendedWatcher::new(move |result: Result<Event>| {
            let event = result.unwrap();

            if event.kind.is_modify() {
                sync_changed_file(&event.paths, &dir_settings);
            } else if event.kind.is_create()  && event.kind == Create(Folder) {
                create_folder_on_remote(&event.paths, &dir_settings);
            } else if event.kind.is_remove() {
                sync_logic::remove_files_and_dirs_from_remote(&event.paths, &dir_settings);
            }
        }, Config::default()
            .with_poll_interval(frequency))?;

    watcher.watch(Path::new(watched_dir.as_str()), RecursiveMode::Recursive)?;


    Ok(())
}

//todo - config should have direct

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use axum::{Json, Router};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::{
        get,
        post
    };
    use axum_test_helper::TestClient;
    use super::*;
    use common::*;

    #[tokio::test]
    async fn test_send_file_to_backend() {
        let server = TestClient::new(router());

        let file_path = PathBuf::from("./test_resources/random_test_files/lophostemon_occurrences.csv");
        std::fs::copy(&file_path, "./example_dir/lophostemon_occurrences.csv").unwrap();
        let file = RemoteFile::new(file_path, "example_dir".to_string(), 0);

        let response = server.post("/copy_to_server").json(&file).send().await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    // a test router for use in testing client
    fn router() -> Router {
        Router::new()
            //Show all files in storage dir
            .route("/", get(common::router_utils::show_files))
            //copy json to storage dir
            .route("/copy_to_server", post(copy_files))
    }

    async fn copy_files(Json(payload): Json<RemoteFile>) -> impl IntoResponse {
        let success = sync_file_with_server(payload).await;
        if success {
            StatusCode::OK
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }

    async fn sync_file_with_server(payload: RemoteFile) -> bool {
        let vaults = vault_utils::deserialize_vault_config();
        let vault = vaults
            .get(&payload.vault_id)
            .unwrap();

        file_utils::copy_file(&payload, &vault).await.unwrap();
        true
    }

}

