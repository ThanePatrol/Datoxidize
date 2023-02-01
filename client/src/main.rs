extern crate core;

mod http_sync;
mod client_db_api;

use notify::*;
use common::common_db_utils;

#[tokio::main]
async fn main() -> Result<()> {
    //init environment variables
    dotenvy::from_path("./client/.env").unwrap();

    // Initial load of db - spawns two lots of pools, gives one to common_utils to read
    // local files and insert/update database accordingly
    // second pool is used for general communication between client and db
    let pool = client_db_api::init_db(
        dotenvy::var("DATABASE_URL")
            .unwrap())
        .await
        .unwrap();
    let pool2 = client_db_api::init_db(
        dotenvy::var("DATABASE_URL")
            .unwrap())
        .await
        .unwrap();
    tokio::task::spawn_blocking(move || {
        common_db_utils::init_metadata_load_into_db(&pool2, false).unwrap();
    })
        .await
        .unwrap();

    let url = reqwest::Url::parse(
        &*dotenvy::var("LOCAL_HOST")
            .unwrap())
        .unwrap();

    // Does initial communication with server, client and url is returned for later reuse
    // local_metadata is read from db, server_data is retrieved from server
    // file_id is the latest key from the servers db, used to update local files
    // that do not exist on server
    let (client, url, local_data, file_id) =
        http_sync::init_metadata_sync(url, &pool).await.unwrap();







    // send_metadata_to_server needs to be called after the initial sync to ensure threads are joined
    //send_metadata_to_server(&client, url, local_data).await;

    //todo - metadata diffing on client to ensure only new files are being inserted
    //client_db_api::insert_server_metadata_into_client_db(&pool, &mut server_data).await.unwrap();




    println!("here");

    //sync_logic::initial_sync(&dir_settings);

    //todo - create a atomic boolean value to limit writes in short succession
    //todo - use a timer to determine when file was last modified
    //todo - if there is another event before this timer reaches 0, reset the timer
    //todo - so if a file is edited many times it will only be synced after a period of inactivity
    /*
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
                old_sync_logic::remove_files_and_dirs_from_remote(&event.paths, &dir_settings);
            }
        }, Config::default()
            .with_poll_interval(frequency))?;

    watcher.watch(Path::new(watched_dir.as_str()), RecursiveMode::Recursive)?;



     */

    Ok(())
}

//todo - config should have direct

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;
    use axum::{Json, Router};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::{
        get,
        post
    };
    use axum_test_helper::TestClient;
    use sqlx::{Pool, Row, Sqlite};
    use super::*;
    use common::*;
    use crate::client_db_api::init_db;

    #[tokio::test]
    async fn test_files_not_added_to_db_multiple_times() {
        let test_file = test_copy_files_to_copy_dir();
        let pool1 = test_init_db().await;
        let pool2 = test_init_db().await;


        let rows = sqlx::query("select * from file_metadata;")
            .fetch_all(&pool2)
            .await
            .unwrap();

        // Should only be one file in the db
        assert_eq!(rows.len(), 1);


        std::fs::remove_file(test_file).unwrap();
        delete_all_from_file_metadata_db(&pool1);
        delete_all_from_file_metadata_db(&pool2);
    }

    #[tokio::test]
    async fn test_init_db_load()  {
        let test_file = test_copy_files_to_copy_dir();
        let pool = test_init_db().await;

        let row = sqlx::query("select * from file_metadata")
            .fetch_one(&pool)
            .await
            .unwrap();

        let file_name = row.get::<String, _>(2);
        assert_eq!(file_name, "/home/hugh/IdeaProjects/Datoxidize/client/example_dir/lophostemon_occurrences.csv".to_string());
        std::fs::remove_file(test_file).unwrap();
        delete_all_from_file_metadata_db(&pool);
    }

    async fn delete_all_from_file_metadata_db(pool: &Pool<Sqlite>) {
        let _ = sqlx::query("delete from file_metadata where true;")
            .execute(pool)
            .await
            .unwrap();
    }

    /// Used for use in testing, deletes all values from DB and returns a pool
    async fn test_init_db() -> Pool<Sqlite> {
        dotenvy::from_path("./.env").unwrap();
        let pool = init_db(dotenvy::var("TEST_DATABASE_URL").unwrap()).await.unwrap();
        pool
    }

    /// Used for copying files from test_resources to the copy directory (example_dir)
    /// for syncing to work
    /// Returns a the path to the newly copied file for deletion after test
    fn test_copy_files_to_copy_dir() -> PathBuf{
        let file_path = PathBuf::from("./test_resources/random_test_files/lophostemon_occurrences.csv");
        std::fs::copy(&file_path, "./example_dir/lophostemon_occurrences.csv").unwrap();
        let copied_file = PathBuf::from("./example_dir/lophostemon_occurrences.csv");
        copied_file
    }

    //todo - Update config to use client.db instead of serialized config
    #[tokio::test]
    async fn test_send_file_to_backend() {
        let server = TestClient::new(router());

        let file_path = PathBuf::from("./test_resources/random_test_files/lophostemon_occurrences.csv");
        std::fs::copy(&file_path, "./example_dir/lophostemon_occurrences.csv").unwrap();
        let copied_file = PathBuf::from("./example_dir/lophostemon_occurrences.csv");
        let file = RemoteFile::new(copied_file.clone(), "example_dir".to_string(), 0, 0);

        let response = server.post("/copy_to_server").json(&file).send().await;
        assert_eq!(response.status(), StatusCode::OK);
        std::fs::remove_file(copied_file).unwrap();
    }

    // a test router for use in testing client
    fn router() -> Router {
        Router::new()
            //Show all files in storage dir
            .route("/", get(router_utils::show_files))
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
        let vaults = config_utils::deserialize_vault_config();
        let vault = vaults
            .get(&payload.vault_id)
            .unwrap();

        file_utils::copy_file_to_server(&payload, &vault).await.unwrap();
        true
    }

}

