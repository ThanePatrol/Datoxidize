use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use axum::routing::get;
use sqlx::{Pool, Row, Sqlite, SqlitePool};
use sqlx::sqlite::{SqlitePoolOptions, SqliteRow};
use tokio::sync::Mutex;
use common::file_utils::{MetadataBlob, FileMetadata, VaultMetadata, ServerPresent};
use common::{common_db_utils, file_utils, RemoteFile};
use crate::ApiState;

/// Main database tables on the server are:
/// 1. file_metadata
/// 2. vaults
///
/// file_metadata has the following columns:
/// 1. file_id - a primary key for identifying every file. This should remain even if a file is deleted
///         rust type is i32, sqlite is INTEGER
/// 2. vault_id - a foreign key for identifying which vault a file belongs to.
///         rust type is i32, sqlite is INTEGER
/// 3. file_path - the full approximate path to a file eg ./vault0/example_file.txt
///         Rust type should be read to string then to PathBuf, sqlite is TEXT
/// 4. modified_time - the last time the file was modified on the server, measured in seconds
///         since unix epoch: i64 for rust, INTEGER for sqlite. An idiomatic way of reading from DB
///         would be read in data as i64.
///         An i64 should be obtained from the file by reading the metadata (provides SystemTime) then
///         `mod_time.duration_since(SystemTime::UNIX_EPOCH)` This provides a Duration struct
///         which should be cast to seconds and stored as i64
/// 5. file_size - the size of the file in bytes
///         Rust type is i64, sqlite is BIGINT
///         NB - file metadata is stored as u64 so has a higher max size than i64
///         This should not be a problem as the maximum size file size that can be stored by i64
///         is approx 9223 PB
///
/// vaults has the following columns:
/// 1. root_dir - the root directory of the vault
///         This must be mirrored across clients - aka every vault they want to sync
///         must have the same root_directory
///         Rust type is String, Sqlite is TEXT
/// 2. sync_frequency - the frequency of syncing actions performed by the client
///         Rust type is i32, sqlite is INTEGER
/// 3. full_path - is the path to the root_dir on the server
///         eg: root_dir is "example_dir" so full_path is "./storage/vault0/example_dir"
///         Rust type is String then PathBuf, sqlite is TEXT
/// 4. vault_id - the primary key, identifies which vault has which key
///         Each vault is identified by a integer so the files can be separated by directory
///         under the same path on the server. eg `./storage` is the vault root
///         the `./storage` dir has folders like `vault0`, `vault1`, etc
///         these allow clean identification of vaults between computers

pub async fn init_db(db_url: String) -> Result<Pool<Sqlite>, Box<dyn Error>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url.as_str())
        .await?;


    Ok(pool)
}

/// Sends metadata blob to client when request by a GET request
/// Reads from DB and maps file metadata to build a structure to be sent via TCP
/// Intended for help in the initial sync of client and server
pub async fn get_metadata_blob(State(state): State<Arc<Mutex<ApiState>>>) -> impl IntoResponse {
    let pool = &state.lock().await.pool;
    let blob = build_metadata_blob(pool)
        .await
        .expect(&*format!("Error reading metadata blob"));
    let id = get_latest_file_id(pool)
        .await
        .expect(&*format!("Error selecting max file_id"));
    Json((id, blob))
}

/// Gets the most recent file_id from db to allow client to update file_ids
/// NB it needs to be incremented before use
async fn get_latest_file_id(pool: &Pool<Sqlite>) -> Result<i32, sqlx::Error> {
    let result = sqlx::query("select file_id from file_metadata order by file_id desc limit 1")
        .fetch_one(pool)
        .await?;
    let latest = result.get::<i32, _>(0);
    Ok(latest)
}

pub async fn insert_new_metadata_into_db(
    State(state): State<Arc<Mutex<ApiState>>>,
    Json(client_blob): Json<MetadataBlob>,
) -> impl IntoResponse {
    let pool = &state.lock().await.pool;
    let files = client_blob.convert_to_metadata_vec();
    common_db_utils::upsert_database(pool, files)
        .await
        .expect(&*format!("Error inserting vec of  \n into database"));
    StatusCode::OK
}

pub async fn get_metadata_differences(
    State(state): State<Arc<Mutex<ApiState>>>,
    Json(client_blob): Json<MetadataBlob>,
) -> impl IntoResponse {
    let pool = &state.lock().await.pool;
    let server_blob = build_metadata_blob(pool)
        .await
        .expect("Error creating server MetadataBlob");

    let difference = file_utils::get_metadata_diff(client_blob, server_blob);
    println!("metadata difference {:?}", difference);
    StatusCode::OK
}

/// Helper function that queries DB and returns a blob of Metadata
async fn build_metadata_blob(pool: &Pool<Sqlite>) -> Result<MetadataBlob, sqlx::Error> {
    let vault_query = sqlx::query("select vault_id from vaults")
        .fetch_all(pool)
        .await?;

    let vaults = vault_query
        .iter()
        .map(|row| row.get::<i32, _>(0))
        .collect::<Vec<i32>>();

    let mut blob = MetadataBlob {
        vaults: HashMap::new(),
    };

    println!("vaults: {:?}", vaults);

    for vault in vaults {
        let query: Vec<SqliteRow> = sqlx::query("select * from file_metadata where vault_id == ? ;")
            .bind(vault)
            .fetch_all(pool)
            .await?;

        let files = map_metadata_query_to_blob(query);
        println!("files: {:?}", files);

        let vault_md = VaultMetadata {
            files,
            vault_id: vault,
        };
        blob.vaults.insert(vault, vault_md);
    }

    Ok(blob)
}

/// A helper method to abstract away the ugly code required to map the rows of data to a vector
fn map_metadata_query_to_blob(rows: Vec<SqliteRow>) -> Vec<FileMetadata> {
    let mut result = Vec::new();
    rows
        .iter()
        .for_each(|row| {
            let file_id = row.get::<i32, _>(0);
            let vault_id = row.get::<i32, _>(1);
            let file_path = row.get::<String, _>(2);
            let root_directory = row.get::<String, _>(3);
            let modified_time = row.get::<i64, _>(4);
            let file_size = row.get::<i64, _>(5);

            let file = FileMetadata {
                full_path: file_path.parse().unwrap(),
                root_directory,
                modified_time,
                file_size,
                vault_id,
                file_id,
                present_on_server: ServerPresent::Yes,
            };
            result.push(file.clone());
        });
    result
}

/*
/// Meant for testing, populate the db with some dummy data to use
pub async fn add_files_to_db(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    let path = PathBuf::from("./backend/storage/vault0/example_dir");
    let paths = file_utils::get_all_files_from_path(&path).unwrap();
    let files = file_utils::test_get_file_metadata_from_path_client(paths.clone());

    println!("files to add:  {:?}", paths);
    for file in files {
        let file_path = file.full_path.into_os_string().into_string().unwrap();
        sqlx::query!(
        "INSERT INTO file_metadata (vault_id, file_path, root_directory, modified_time, file_size)
        VALUES ($1, $2, $3, $4, $5);",
            file.vault_id, file_path,
            file.root_directory, file.modified_time, file.file_size)
            .execute(pool)
            .await?;
    }
    Ok(())
}

 */

