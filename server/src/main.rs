mod html_creation;
mod sync_core;

use axum::{
    routing::{get, post},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use serde_json::{json, Value};
use crate::sync_core::RemoteFile;

#[tokio::main]
async fn main() {
    //init environment variables
    dotenvy::dotenv().unwrap();

    let m_data = std::fs::metadata("./templates/directory.html").unwrap();
    println!("{:?}", m_data);
    let file = std::fs::read("./templates/directory.html").unwrap();


    tracing_subscriber::fmt::init();

    // Building application routes
    let router = router();

    // listening on localhost
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::debug!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await.unwrap();
}

fn router() -> Router {
    Router::new()
        // `GET /` goes to `root`
        .route("/", get(show_files))
        // 'GET /show' will display the content posted in /test
        .route("/show", get(get_synced_file))
        // GET show_dirs will show the current list of directories being watched
        .route("/show_dirs", get(get_directories))
        // POST /copy takes a JSON form of a file and copies it to the server
        .route("/copy", post(copy_file))
}

//The argument tells axum to parse request as JSON into RemoteFile
async fn copy_file(Json(payload): Json<RemoteFile>) -> impl IntoResponse {
    let success = sync_core::sync_file_with_server(payload).await;
    if success {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
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

/*
async fn sync_file(Json(payload): Json<FileRaw>) -> impl IntoResponse {
    //convert the raw data from front end into a
    let string_form = std::str::from_utf8(&*payload.content).unwrap();
    let file = FileHumanReadable {
        content: string_form.to_string(),
    };

    //converted
    (StatusCode::CREATED, Json(file))
}

 */

async fn get_synced_file() -> Json<Value> {
    let file = get_file_from_database();
    Json(json!(file))
}

pub fn get_file_from_database() -> String {
    std::fs::read_to_string("./storage/test.txt").expect("")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use axum::extract::Path;
    use super::*;
    use axum::http::StatusCode;
    use axum_test_helper::TestClient;

    #[tokio::test]
    async fn copy_file_via_http() {
        let router = router();
        let client = TestClient::new(router);
        let path = PathBuf::from("../datoxidize/example_dir/lophostemon_occurrences.csv");
        let metadata = std::fs::metadata(path.clone()).unwrap();
        let file = RemoteFile {
            full_path: path.clone(),
            root_directory: "example_dir".to_string(),
            contents: std::fs::read(path.clone()).unwrap(),
            metadata: (metadata.accessed().unwrap(), metadata.modified().unwrap(), metadata.len()),
            vault_id: 0,
        };

        let response = client.post("/copy").json(&file).send().await;
        assert_eq!(response.status(), StatusCode::OK);
        let final_path = std::path::Path::new("./storage/vault0/lophostemon_occurrences.csv");
        assert!(final_path.exists());
        std::fs::remove_file("./storage/vault0/lophostemon_occurrences.csv").unwrap();
    }
}