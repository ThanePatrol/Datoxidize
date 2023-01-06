mod html_creation;
mod sync_core;
mod db_api;

use std::error::Error;
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
use common::{db_utils, RemoteFile};
use common::file_utils::FileMetadata;
use sync_core::sync_file_with_server;
use crate::db_api::{get_metadata_blob, get_metadata_differences};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //init environment variables
    dotenvy::from_path("./backend/.env").unwrap();

    //init db
    let pool = db_api::init_db(var("DATABASE_URL").unwrap()).await?;
    let pool2 = db_api::init_db(var("DATABASE_URL").unwrap()).await?;
    tokio::task::spawn_blocking(move || {
        db_utils::init_metadata_load_into_db(&pool2, true);
    }).await.unwrap();
    println!("loaded metadata into db");
    //db_api::add_files_to_db(&pool).await?;
    //let file = fs::read("./templates/directory.html").unwrap();

    tracing_subscriber::fmt::init();

    // Building application routes
    let router = router(pool);

    // listening on localhost
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::debug!("listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await?;
    Ok(())
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
        // GET /copy/metadata_blob_send gets the files as a metadata blob struct as json and sends to client
        .route("/copy/metadata_blob_send", get(get_metadata_blob))
        //POST /copy/metadata_blob_receive receives the files as a metadata blob from client, this is part of the initial handshake
        .route("/copy/metadata_blob_receive", post(get_metadata_differences))

        .with_state(pool)
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
    use std::{env, fs, io, path};
    use std::path::PathBuf;
    use super::*;
    use axum::http::StatusCode;
    use axum_test_helper::TestClient;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn copy_file_via_http() {
        let pool = test_db_init().await;
        let router = router(pool);
        let client = TestClient::new(router);
        let path = PathBuf::from("../client/example_dir/test_file_http/lophostemon_occurrences.csv");
        fs::copy("../client/test_resources/random_test_files/lophostemon_occurrences.csv",
        &path).unwrap();
        let file = RemoteFile::new(path, "example_dir".to_string(), 0);

        let response = client.post("/copy").json(&file).send().await;
        assert_eq!(response.status(), StatusCode::OK);
        let final_path = path::Path::new("./storage/vault0/test_file_http/lophostemon_occurrences.csv");
        assert!(final_path.exists());
        remove_dir_contents("./storage/vault0/test_file_http").unwrap();
        remove_dir_contents("../client/example_dir/test_file_http").unwrap();
    }

    async fn test_db_init() -> Pool<Sqlite> {
        println!("dir: {:?}", env::current_dir());
        dotenvy::from_path("./.env").unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(dotenvy::var("TEST_DATABASE_URL").unwrap().as_str())
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn copy_nested_file_via_http() {
        let pool = test_db_init().await;
        let router = router(pool);
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