use std::fs;
use std::path::{Path, PathBuf};
use crate::RemoteFile;
use axum::{
    routing::{get, post},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};

pub fn router() -> Router {
    Router::new()
        //Show all files in storage dir
        .route("/", get(show_files))
        //copy json to storage dir
        .route("/sync", post(copy_files))
}

pub async fn show_files() -> String {
    let files = std::fs::read_dir("./storage").unwrap();
    let mut files_as_string = String::new();
    for file in files {
        files_as_string.push_str("| ");
        files_as_string.push_str(file.unwrap().file_name().to_str().unwrap());
        files_as_string.push_str("\n")
    }
    files_as_string
}

pub async fn copy_files(Json(payload): Json<RemoteFile>) -> impl IntoResponse {
    let success = sync_file_with_server(payload).await;
    if success {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

pub async fn sync_file_with_server(payload: RemoteFile) -> bool {
    false
}



/*
pub async fn post_files_test(Json(payload): Json<RemoteFile>) -> impl IntoResponse {
    let success = sync_core::sync_file_with_server(payload).await;
    if success {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

pub async fn sync_files_to_server(payload: RemoteFile) -> bool {

}

 */

