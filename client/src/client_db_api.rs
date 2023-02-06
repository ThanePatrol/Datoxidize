use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{ConnectOptions, Pool, Row, Sqlite, SqlitePool};
use common::{common_db_utils, file_utils};
use common::common_db_utils::upsert_database;
use common::file_utils::{FileMetadata, MetadataBlob, ServerPresent, VaultMetadata};

pub async fn init_db(db_url: String) -> Result<Pool<Sqlite>, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url.as_str())
        .await?;

    println!("Pool loaded");

    Ok(pool)
}

/// Accepts server metadata and inserts it into client db. Assumes server metadata consists of new files
/// not present on local
pub async fn insert_server_metadata_into_client_db(pool: &Pool<Sqlite>, server_mdata: &mut MetadataBlob) -> Result<(), sqlx::Error> {
    let files = file_utils::convert_blob_to_vec_metadata(server_mdata);
    println!("files new for client: {:?}", files);
    upsert_database(pool, files).await?;
    Ok(())
}

/// Loads metadata from DB into a MetadataBlob struct to send to the server
pub async fn load_file_metadata(pool: &Pool<Sqlite>, file_id: i32) -> Result<MetadataBlob, sqlx::Error> {
    let vaults = get_all_vaults(pool).await?;
    let mut blob = MetadataBlob {
        vaults: HashMap::new(),
    };

    let mut cur_id = file_id;
    for vault in vaults {
        let rows = sqlx::query("select file_id, file_path, root_directory, modified_time, file_size from file_metadata where vault_id == ?;")
            .bind(vault)
            .fetch_all(pool)
            .await?;

        let (id, files) = build_file_metadata_for_vault(vault, rows, cur_id);
        cur_id = id;

        let vault_metadata = VaultMetadata {
            files,
            vault_id: vault,
        };

        blob.vaults.insert(vault, vault_metadata);
    }

    Ok(blob)
}

async fn get_all_vaults(pool: &Pool<Sqlite>) -> Result<Vec<i32>, sqlx::Error> {
    let rows = sqlx::query("select vault_id from vaults;")
        .fetch_all(pool)
        .await?;

    let vaults = rows
        .iter()
        .map(|row| row.get::<i32, _>(0))
        .collect::<Vec<i32>>();
    Ok(vaults)
}

fn build_file_metadata_for_vault(vault: i32, rows: Vec<SqliteRow>, latest_file_id: i32) -> (i32, Vec<FileMetadata>) {
    let mut files = Vec::new();

    let mut id = latest_file_id;

    rows
        .iter()
        .for_each(|row| {
            let path = PathBuf::from(row.get::<String, _>(1));
            let mut absolute_root_dir = path.clone();
            absolute_root_dir.pop();

            let mut file = FileMetadata {
                full_path: path,
                root_directory: row.get::<String, _>(2),
                absolute_root_dir,
                modified_time: row.get::<i64, _>(3),
                file_size: row.get::<i64, _>(4),
                vault_id: vault,
                /// Match the file_id, if equal to -1, update it with new id and increment
                file_id: match row.get::<i32, _>(0) {
                    -1 => {
                        id += 1;
                        id
                    }
                    x => x,
                },
                present_on_server: ServerPresent::Unknown,
            };
            if row.get::<i32, _>(0) == -1 {
                file.present_on_server = ServerPresent::No;
            } else {
                file.present_on_server = ServerPresent::Yes;
            }
            files.push(file);
        });
    (id, files)
}

