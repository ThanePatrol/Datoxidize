mod html_creation;

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
async fn main() {
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

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await.unwrap();
}


async fn get_directories() -> impl IntoResponse {
    html_creation::test_render().await
}

async fn show_files() -> String {
    let files = std::fs::read_dir("./storage").unwrap();
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
    std::fs::read_to_string("./storage/test.txt").expect("")
}