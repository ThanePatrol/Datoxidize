use std::collections::HashSet;
use std::error::Error;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use axum::routing::get;
use sqlx::{Pool, Row, Sqlite, SqlitePool};
use sqlx::sqlite::{SqlitePoolOptions, SqliteRow};
use common::file_utils::RemoteFileMetadata;
use common::RemoteFile;

pub async fn init_db(db_url: String) -> Result<Pool<Sqlite>, Box<dyn Error>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url.as_str())
        .await?;

    let mut files = get_all_files_on_server(&pool).await?;
    let sorted_files = sort_files_by_modified_time(&mut files);
    Ok(pool)
}

/// Is part the main REST API, will receive the clients files, check metadata
/// Responds with a 200 if all files are up to date, if client needs newer files then a 210
/// will be sent along with a list of files for the client to update
/// If the server needs newer files it will send a 211 with the list of files it needs updates on
/// NB the vec should be sorted by file_id, (lowest first - This is because a RemoteFileMetadata struct
/// with a file_id of -1 is not present on the server) then by metadata access time
///
pub async fn init_client_sync(
    State(pool): State<Pool<Sqlite>>,
    Json(payload): Json<Vec<Vec<RemoteFileMetadata>>>) -> impl IntoResponse {

    //todo - where i got up to: take file differences, get request to client for new files
    // then send client the new files for to save
    let server_files = match get_all_files_on_server(&pool).await {
        Ok(files) => files,
        Err(e) => panic!("Problem reading from data base: {:?}", e),
    };
    StatusCode::OK
}

/// Returns a tuple with 0th being a `Vec<Vec<&RemoteFileMetadata>>` which are more recent on the client
/// the 1st is a `Vec<Vec<&RemoteFileMetadata>>` which are more recent on the server
//todo create a struct for MinimalRemoteMetadata that only stores the file_id
// and try to create a solution that is not O(n^2) / some horrible mess
async fn get_file_differences(
    client_files: Vec<Vec<RemoteFileMetadata>>,
    server_files: Vec<Vec<RemoteFileMetadata>>) -> (Vec<Vec<&RemoteFileMetadata>>, Vec<Vec<&RemoteFileMetadata>>) {

    let n_vaults = client_files.len();
    let mut new_for_client = vec![vec![]; n_vaults];
    let mut new_for_server = vec![vec![]; n_vaults];


    for i in 0..n_vaults {
        let mut seen_on_client = HashSet::new();
        let mut seen_on_server = HashSet::new();

        for client_file in client_files[i] {
            if client_file.file_id == -1 {
                new_for_server[i].push(&client_file);
                continue;
            }

            seen_on_client.insert(client_file.file_id);

            for server_file in server_files[i] {
                if client_file.file_id != server_file.file_id {
                    continue;
                }

                if client_file.metadata.1 > server_file.metadata.1 {
                    new_for_server[i].push(&client_file);
                }
            }
        }

        for server_file in server_files[i] {
            seen_on_server.insert(server_file.file_id);
            for client_file in client_files[i] {
                if server_file.file_id != client_file.file_id {
                    continue;
                }
                if server_file.metadata.1 > client_file.metadata.1 {
                    new_for_client[i].push(&server_file);
                }
            }
        }

        let only_on_server = seen_on_server.difference(&seen_on_client);

        for srv_only in only_on_server {
            for srv_file in server_files[i] {
                if srv_file.file_id == srv_only {
                    new_for_client[i].push(&srv_file);
                }
            }
        }
    }
    (new_for_client, new_for_server)
}

/// Queries database and gets all the files in the form of a `Vec<VecRemoteFileMetadata>>`
/// The first vec is a list of files for a particular vault, eg files for vault0 are in the 0th index
/// The nested vec is a list of the actual file details for a vault
async fn get_all_files_on_server(pool: &Pool<Sqlite>) -> Result<Vec<Vec<RemoteFileMetadata>>, Box<dyn Error>> {
    let vault_max: SqliteRow = sqlx::query("select max(vault_id) from vaults")
        .fetch(pool)
        .await?;

    let vault_num: i32 = vault_max.get(0);

    let rows: Vec<SqliteRow> = sqlx::query("select * from file_metadata;")
        .fetch_all(pool)
        .await?;

    let mut files = Vec::with_capacity(vault_num as usize + 1);

    for row in rows {
        let cur_vault = row.get(1);
        let file = RemoteFileMetadata {
            full_path: row.get(2),
            root_directory: row.get(3),
            metadata: (row.get(4), row.get(5), row.get(6)),
            vault_id: cur_vault,
            file_id: row.get(0),
        };
        files[cur_vault].push(file)
    }
    Ok(files)
}

/// Sorts by metadata access time - most recent metadata comes first
fn sort_files_by_modified_time(files: &mut Vec<Vec<RemoteFileMetadata>>) -> &mut Vec<Vec<RemoteFileMetadata>> {
    for vault in files {
        vault.sort_by(|a, b| {
            b.metadata.1.cmp(&a.metadata.1)
        });
    }
    files
}