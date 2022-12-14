mod html_creation;
mod sync_core;
mod db_api;

use std::fs;
use axum::{
    routing::{get, post},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use std::net::SocketAddr;
use axum::extract::State;
use dotenvy::{dotenv, var};
use serde_json::{json, Value};
use sqlx::{Pool, Sqlite};
use common::{RemoteFile};
use common::file_utils::RemoteFileMetadata;
use sync_core::sync_file_with_server;
use crate::db_api::init_client_sync;

#[tokio::main]
async fn main() {
    //init environment variables
    dotenvy::from_path("./backend/.env").unwrap();
    let pool = db_api::init_db(var("DATABASE_URL").unwrap()).await.unwrap();

    //let file = fs::read("./templates/directory.html").unwrap();

    tracing_subscriber::fmt::init();

    // Building application routes
    let router = router(pool);

    // listening on localhost
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::debug!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await.unwrap();
}

fn router(pool: Pool<Sqlite>) -> Router {
    Router::new()
        // `GET /` goes to `root`
        .route("/", get(common::router_utils::show_files))
        // 'GET /show' will display the content posted in /test
        .route("/show", get(get_synced_file))
        // GET show_dirs will show the current list of directories being watched
        .route("/show_dirs", get(get_directories))
        // POST /copy takes a JSON form of a file and copies it to the server
        .route("/copy", post(copy_file))
        //POST /copy/init takes JSON RemoteFileMetadata and responds with various HTTP codes depending on what needs to be done next
        .route("/copy/init", post(init_sync))

        .with_state(pool)
}

async fn init_sync(State(pool): State<Pool<Sqlite>>, Json(payload): Json<Vec<Vec<RemoteFileMetadata>>>) -> impl IntoResponse {
    println!("hit init_sync");
    let code = init_client_sync(&pool, payload).await;
    code
}

//The argument tells axum to parse request as JSON into RemoteFile
async fn copy_file(Json(payload): Json<RemoteFile>) -> impl IntoResponse {
    let path = payload.full_path.clone();
    let success = sync_file_with_server(payload).await;
    if success {
        println!("saved file successfully :) {:?}", path);
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn get_directories() -> impl IntoResponse {
    html_creation::test_render().await
}


async fn get_synced_file() -> Json<Value> {
    let file = "";
    Json(json!(file))
}


#[cfg(test)]
mod tests {
    use std::{fs, io, path};
    use std::path::PathBuf;
    use super::*;
    use axum::http::StatusCode;
    use axum_test_helper::TestClient;

    #[tokio::test]
    async fn copy_file_via_http() {
        let router = router();
        let client = TestClient::new(router);
        let path = PathBuf::from("../client/example_dir/test_file_http/lophostemon_occurrences.csv");
        fs::copy("../client/test_resources/random_test_files/lophostemon_occurrences.csv",
        &path).unwrap();
        let file = common::RemoteFile::new(path, "example_dir".to_string(), 0);

        let response = client.post("/copy").json(&file).send().await;
        assert_eq!(response.status(), StatusCode::OK);
        let final_path = path::Path::new("./storage/vault0/test_file_http/lophostemon_occurrences.csv");
        assert!(final_path.exists());
        remove_dir_contents("./storage/vault0/test_file_http").unwrap();
        remove_dir_contents("../client/example_dir/test_file_http").unwrap();
    }

    #[tokio::test]
    async fn copy_nested_file_via_http() {
        let router = router();
        let client = TestClient::new(router);
        fs::create_dir_all("../client/example_dir/test_copy_nested_http/http_test/another").unwrap();
        let file_path = "../client/example_dir/test_copy_nested_http/http_test/another/test.csv";
        fs::File::create(file_path).unwrap();
        fs::write(file_path, "test,content,string".to_string()).unwrap();

        let file = common::RemoteFile::new(PathBuf::from(file_path), "example_dir".to_string(), 0);

        let response = client.post("/copy").json(&file).send().await;
        assert_eq!(response.status(), StatusCode::OK);
        let copied_path = path::Path::new("./storage/vault0/test_copy_nested_http/http_test/another/test.csv");
        assert_eq!(fs::read(copied_path).unwrap(), file.contents);

        remove_dir_contents("./storage/vault0/test_copy_nested_http").unwrap();
        remove_dir_contents("../client/example_dir/test_copy_nested_http").unwrap();
    }

    fn remove_dir_contents<P: AsRef<path::Path>>(path: P) -> io::Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if entry.file_type()?.is_dir() {
                remove_dir_contents(&path)?;
                fs::remove_dir(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}