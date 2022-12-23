mod sync_logic;
mod gui;

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
use notify::EventKind::Create;
use serde_json::{json, Value};
use crate::gui::HtmlTemplate;

#[tokio::main]
async fn main() -> Result<()> {
    //init environment variables
    dotenvy::dotenv().unwrap();

    tracing_subscriber::fmt::init();

    // Building application routes
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(show_files))
        // `POST /users` goes to `create_user`
        .route("/test", post(sync_file))
        // 'GET /show' will display the content posted in /test
        .route("/show", get(get_synced_file))
        // GET show_dirs will show the current list of directories being watched
        .route("/show_dirs", get(get_directories));

    // listening on localhost
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
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
                create_folder_on_remote(&event.paths, &dir_settings);
            } else if event.kind.is_remove() {
                sync_logic::remove_files_and_dirs_from_remote(&event.paths, &dir_settings);
            }
        },notify::Config::default()
                                    .with_poll_interval(Duration::from_secs(1)))?;

    watcher.watch(Path::new(watched_dir.as_str()), RecursiveMode::Recursive)?;
    //todo - set duration::from_secs() from user preferences.

    gui::test_print_html();

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await.unwrap();

    Ok(())
}

async fn get_directories() -> impl IntoResponse {
    gui::test_render().await
}

async fn show_files() -> String {
    let files = std::fs::read_dir("./copy_dir").unwrap();
    let mut files_as_string = String::new();
    for file in files {
        files_as_string.push_str("| ");
        files_as_string.push_str(file.unwrap().file_name().to_str().unwrap());
        files_as_string.push_str("\n")
    }
    files_as_string
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
