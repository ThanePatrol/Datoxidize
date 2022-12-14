mod web_server;
mod sync_logic;

use std::path::Path;
use notify::*;
use std::sync::mpsc::channel;
use std::time::{Duration, SystemTime};
use notify::event::{AccessKind, AccessMode};
use std::fs;
use crate::sync_logic::{DirectorySettings, sync_directory};
use axum::{
    routing::{get, post},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<()>{
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

    let CONFIG_PATH = "./example_dir";

    println!("waiting");

    // listening on localhost
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);

    //todo - Load these settings from a config.json file
    //Create DirectorySettings
    let dir_settings = DirectorySettings::new(
        "./example_dir".to_string(), 1, 1);


    //NB - This watcher needs to be initiated like this to allow for asynchronous runtime be
    //between web server and notifications

    let mut watcher =
        // To make sure that the config lives as long as the function
        // we need to move the ownership of the config inside the function
        // To learn more about move please read [Using move Closures with Threads](https://doc.rust-lang.org/book/ch16-01-threads.html?highlight=move#using-move-closures-with-threads)
        RecommendedWatcher::new(move |result: Result<Event>| {
            let event = result.unwrap();

            if event.kind.is_modify() {
                sync_directory(event, &dir_settings);
            }
        },notify::Config::default()
            .with_poll_interval(Duration::from_secs(1)))?;

    watcher.watch(Path::new(CONFIG_PATH), RecursiveMode::Recursive)?;

    //todo - set duration::from_secs() from user preferences

    println!("watching");
    // Main event loop, will loop forever and call syncing functions
    //for event in rx {
    //    let e = event.unwrap();
    //    println!("{:?}", e);
    //    if let EventKind::Access(AccessKind::Close(AccessMode::Write)) = e.kind {
    //        let start = SystemTime::now();
    //        sync_logic::sync_directory(e, &dir_settings);
    //        println!("synced");
    //        let end = SystemTime::now();
    //        let time = end.duration_since(start).unwrap();
    //        println!("{:?}", time);
    //    }
//
    //}
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

fn sync_file_to_local(event: Event) {
    let file_name = event.paths[0].file_name().unwrap();
    let data = fs::read(event.paths[0].as_path()).expect("");
    let sync_path = "./copy_dir/".to_string() + file_name.to_str().unwrap();
    println!("sync: {}", sync_path);
    fs::write(sync_path, data).expect("Error syncing data");
}

fn sync_file_to_remote(event: Event) {
    let file_name = event.paths[0].file_name().unwrap();
    let data = fs::read(event.paths[0].as_path()).expect("");
    //let raw_file_json = F
}

pub fn get_file_from_database() -> String {
    fs::read_to_string("./copy_dir/test").expect("")
}

