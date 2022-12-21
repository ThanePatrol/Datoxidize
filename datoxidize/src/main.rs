mod sync_logic;

use std::path::Path;
use notify::*;
use std::time::{Duration};
use std::fs;
use crate::sync_logic::{create_folder_on_remote, deserialize_config, sync_changed_file};
use axum::{
    routing::{get, post},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use notify::event::CreateKind::Folder;
use notify::event::RemoveKind::File;
use notify::EventKind::Create;
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<()> {
    //init environment variables
    dotenvy::dotenv().unwrap();

    tracing_subscriber::fmt::init();

    // Building application routes
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        // `POST /users` goes to `create_user`
        .route("/test", post(sync_file))
        // 'GET /show' will display the content posted in /test
        .route("/show", get(get_synced_file));

    // listening on localhost
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);

    let dir_settings = deserialize_config("./test_resources/config.json".to_string()).unwrap();
    let watched_dir = &dir_settings.content_directory.clone();

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
                println!("{:?}", event);
                create_folder_on_remote(&event.paths, &dir_settings);
            } else if event.kind.is_remove() {
                sync_logic::remove_files_and_dirs_from_remote(&event.paths, &dir_settings);
            }
        },notify::Config::default()
                                    .with_poll_interval(Duration::from_secs(1)))?;

    watcher.watch(Path::new(watched_dir.as_str()), RecursiveMode::Recursive)?;
    //todo - set duration::from_secs() from user preferences.

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await.unwrap();
    Ok(())
}

async fn root() -> &'static str {
    "Hello World!"
}

//The argument tells axum to parse request as JSON into FileRaw
async fn sync_file(Json(payload): Json<FileRaw>) -> impl IntoResponse {
    //convert the raw data from front end into a
    let string_form = std::str::from_utf8(&*payload.content).unwrap();
    let file = FileHumanReadable {
        content: string_form.to_string(),
    };

    //converted
    (StatusCode::CREATED, Json(file))
}

async fn get_synced_file() -> Json<Value> {
    let file = get_file_from_database();
    Json(json!(file))
}

//the input for the sync_file function
#[derive(Deserialize)]
struct FileRaw {
    content: Vec<u8>
}

//the output for the sync_file function
#[derive(Serialize)]
struct FileHumanReadable {
    content: String,
}

pub fn get_file_from_database() -> String {
    fs::read_to_string("./copy_dir/test").expect("")
}

/*
#[cfg(test)]
///Current unit tests cover:
/// 1. Making a new directory
/// 2. Syncing file content with a watcher
mod tests {
    use std::thread;
    use crate::sync_logic::{create_new_remote_directory};
    use super::*;


    #[test]
    fn test_make_new_directory_remote() {
        println!("Test");
        let directory_path_to_watch = "example_dir";
        //Create DirectorySettings
        let dir_settings = DirectoryConfig::new(
            directory_path_to_watch.to_string(), 1, Duration::from_secs(1));

        let mut new_dir_path = dotenvy::var("TEST_DIRECTORY").unwrap();
        new_dir_path.push_str("testing");

        std::fs::create_dir(new_dir_path.clone()).expect("Error creating dir");
        let new_remote_dir = //get_new_remote_directory_path(new_dir_path.clone(), &dir_settings);
        create_new_remote_directory(new_remote_dir.clone());

        let path_remote = Path::new(&new_remote_dir).file_name().unwrap();
        let path_local = Path::new(&new_dir_path).file_name().unwrap();

        assert_eq!(path_local, path_remote);
        assert!(Path::new(&new_remote_dir).exists());

        std::fs::remove_dir(new_dir_path).expect("");
        std::fs::remove_dir(new_remote_dir).expect("");
    }


    // todo - update tests and/or methods to instantiate a simple custom watcher to use for tests
    #[test]
    fn test_content_syncs_with_remote() {
        //Create watcher boilerplate
        let dir_settings = deserialize_config("./test_resources/config.json".to_string()).unwrap();
        let watched_dir = dir_settings.content_directory.clone();
        let cloned_config = dir_settings.clone();
        let mut watcher =
            RecommendedWatcher::new(move |result: Result<Event>| {
                let event = result.unwrap();

                if event.kind.is_modify() {
                    sync_changed_file(event, &dir_settings);
                } else if event.kind.is_create()  && event.kind == Create(Folder) {
                    sync_logic::get_new_remote_directory_path(event.paths[0].as_path().to_str().unwrap().to_string(),
                                                              &dir_settings);
                } else if event.kind.is_remove() {
                    println!("{event:?}");
                }
            },notify::Config::default()
                                        .with_poll_interval(Duration::from_secs(1))).unwrap();
        watcher.watch(Path::new(watched_dir.as_str()), RecursiveMode::Recursive).unwrap();

        let test_file = "test_content.txt";

        let mut new_file_path = watched_dir;
        new_file_path.push_str("/");
        new_file_path.push_str(test_file);
        let content = "This is a unit test";
        std::fs::write(new_file_path.clone(), content).expect("unable to write file");
        thread::sleep(Duration::from_millis(1001));


        let mut sync_path = cloned_config.remote_relative_directory;
        sync_path.push_str(cloned_config.content_directory.as_str());
        sync_path.push_str("/");
        sync_path.push_str(test_file);
        let synced_content = std::fs::read_to_string(sync_path.clone()).unwrap();

        assert_eq!(content, synced_content);
        std::fs::remove_file(sync_path).unwrap();
        std::fs::remove_file(new_file_path).unwrap();

    }



    //#[test]
    fn test_create_and_delete_file() {
        let watcher = create_watcher();

    }

    fn create_watcher() -> INotifyWatcher {
        let dir_settings = deserialize_config("./test_resources/config.json".to_string()).unwrap();
        let watched_dir = dir_settings.content_directory.clone();
        let mut watcher =
            RecommendedWatcher::new(move |result: Result<Event>| {
                let event = result.unwrap();

                if event.kind.is_modify() {
                    sync_logic::sync_changed_file(event, &dir_settings);
                } else if event.kind.is_create()  && event.kind == Create(Folder) {
                    sync_logic::create_new_remote_directory(event, &dir_settings)
                } else if event.kind.is_remove() {
                    sync_logic::remove_file_from_remote(event, &dir_settings);
                }
            },notify::Config::default()
                                        .with_poll_interval(Duration::from_secs(1))).unwrap();

        watcher.watch(Path::new(watched_dir.as_str()), RecursiveMode::Recursive).unwrap();
        watcher
    }
}

 */
