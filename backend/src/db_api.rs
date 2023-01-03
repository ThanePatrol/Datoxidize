use std::collections::HashSet;
use std::error::Error;
use std::time::{Duration, SystemTime};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use axum::routing::get;
use sqlx::{Pool, Row, Sqlite, SqlitePool};
use sqlx::sqlite::{SqlitePoolOptions, SqliteRow};
use common::file_utils::RemoteFileMetadata;
use common::RemoteFile;

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
///         An i32 should be obtained from the file by reading the metadata (provides SystemTime) then
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

    //let mut files = get_all_files_on_server(&pool).await?;
    //sort_files_by_modified_time(&mut files);
    Ok(pool)
}

/// Is part the main REST API, will receive the clients files, check metadata
/// Responds with a 200 if all files are up to date, if client needs newer files then a 210
/// will be sent along with a list of files for the client to update
/// If the server needs newer files it will send a 211 with the list of files it needs updates on
/// NB the vec should be sorted by file_id, (lowest first - This is because a RemoteFileMetadata struct
/// with a file_id of -1 is not present on the server) then by metadata access time
/// A status code of 500 means problem with reading data
pub async fn init_client_sync(
    pool: &Pool<Sqlite>,
    payload: Vec<Vec<RemoteFileMetadata>>) -> impl IntoResponse {
    println!("route reached");

    //todo - where i got up to: take file differences, get request to client for new files
    // then send client the new files for to save
    let server_files = match get_all_files_on_server(&pool).await {
        Ok(files) => files,
        Err(e) => panic!("Problem reading from data base: {:?}", e),
    };

    println!("payload is: {:?}", payload);
    println!("server files are: {:?}", server_files);
    let differences = get_file_differences(payload, server_files);
    println!("differences are: {:?}", differences);

    StatusCode::INTERNAL_SERVER_ERROR
}

/// Returns a tuple with 0th being a `Vec<Vec<&RemoteFileMetadata>>` which are more recent on the client
/// the 1st is a `Vec<Vec<&RemoteFileMetadata>>` which are more recent on the server
//todo create a struct for MinimalRemoteMetadata that only stores the file_id
// and try to create a solution that is not O(n^2) / some horrible mess
fn get_file_differences(
    client_files: Vec<Vec<RemoteFileMetadata>>,
    server_files: Vec<Vec<RemoteFileMetadata>>) -> (Vec<Vec<RemoteFileMetadata>>, Vec<Vec<RemoteFileMetadata>>) {

    let n_vaults = client_files.len();
    let mut new_for_client = vec![vec![]; n_vaults];
    let mut new_for_server = vec![vec![]; n_vaults];


    for i in 0..n_vaults {
        let mut seen_on_client = HashSet::new();
        let mut seen_on_server = HashSet::new();

        for client_file in client_files[i].iter() {
            if client_file.file_id == -1 {
                new_for_server[i].push(client_file.clone());
                continue;
            }

            seen_on_client.insert(client_file.file_id);

            for server_file in server_files[i].iter() {
                if client_file.file_id != server_file.file_id {
                    continue;
                }

                if client_file.metadata.1 > server_file.metadata.1 {
                    new_for_server[i].push(client_file.clone());
                }
            }
        }

        for server_file in server_files[i].iter() {
            seen_on_server.insert(server_file.file_id);
            for client_file in client_files[i].iter() {
                if server_file.file_id != client_file.file_id {
                    continue;
                }
                if server_file.metadata.1 > client_file.metadata.1 {
                    let copy = server_file.clone();
                    new_for_client[i].push(copy);
                }
            }
        }

        let only_on_server = seen_on_server.difference(&seen_on_client);

        for srv_only in only_on_server {
            for srv_file in server_files[i].iter() {
                if &srv_file.file_id == srv_only {
                    new_for_client[i].push(srv_file.clone());
                }
            }
        }
    }
    (new_for_client, new_for_server)
}

/// Queries database and gets all the files in the form of a `Vec<VecRemoteFileMetadata>>`
/// The first vec is a list of files for a particular vault, eg files for vault0 are in the 0th index
/// The nested vec is a list of the actual file details for a vault
async fn get_all_files_on_server(pool: &Pool<Sqlite>) -> Result<Vec<Vec<RemoteFileMetadata>>, Box<sqlx::Error>> {
    //let vault_max = sqlx::query!("select * from vaults;")
    //    .fetch_one(pool)
    //    .await?;


    //let vault_num: i32 = vault_max.get(0);

    let rows: Vec<SqliteRow> = sqlx::query("select * from file_metadata;")
        .fetch_all(pool)
        .await?;

    let mut files = Vec::new();

    //let cur_vault = row

    for row in rows {
        let cur_vault = row.get::<i32, _>(1);
        println!(cur_vault);

        /*
        let access_time = SystemTime::from(Duration::from_secs(row.get::<i32, _>(4) as u64));
        let file = RemoteFileMetadata {
            full_path: row.get(2),
            root_directory: row.get(3),
            metadata: (SystemTime::frrow.get(4), row.get(5), row.get(6)),
            vault_id: cur_vault,
            file_id: row.get::<i32, _>(0),
        };
        files[cur_vault].push(file)

         */
    }


    Ok(files)
}

/// Sorts by metadata access time - most recent metadata comes first
fn sort_files_by_modified_time(files: &mut Vec<Vec<RemoteFileMetadata>>) {
    for vault in files {
        vault.sort_by(|a, b| {
            b.metadata.1.cmp(&a.metadata.1)
        });
    }
}

pub fn get_vault_config_hashmap() {

}