use axum::{
    routing::{get, post},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use serde_json::{json, Value};
use crate::get_file_from_database;

#[tokio::main]
pub async fn init_web_server() {
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        // `POST /users` goes to `create_user`
        .route("/test", post(sync_file))
        // 'GET /show' will display the content posted in /test
        .route("/show", get(get_synced_file));


    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
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