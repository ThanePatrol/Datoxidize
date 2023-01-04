use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::{ConnectOptions, Pool, Row, Sqlite, SqlitePool};
use common::file_utils::{FileMetadata, MetadataBlob, VaultMetadata};

pub async fn init_db(db_url: String) -> Result<Pool<Sqlite>, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url.as_str())
        .await?;
    Ok(pool)
}

pub async fn load_file_metadata(pool: &Pool<Sqlite>) -> Result<MetadataBlob, sqlx::Error> {
    let vaults = get_all_vaults(pool).await?;
    let mut blob = MetadataBlob {
        vaults: HashMap::new(),
    };

    for vault in vaults {
        let rows = sqlx::query("select file_id, file_path, root_directory, modified_time, file_size from file_metadata where vault_id == ?;")
            .bind(vault)
            .fetch_all(pool)
            .await?;

        let files = build_file_metadata_for_vault(vault, rows);

        let vault_metadata = VaultMetadata {
            files,
            vault_id: vault,
        };

        blob.vaults.insert(vault, vault_metadata);
    }

    Ok(blob)
}

async fn get_all_vaults(pool: &Pool<Sqlite>) -> Result<Vec<i32>, sqlx::Error> {
    let rows = sqlx::query("select vault_id from client_vaults;")
        .fetch_all(pool)
        .await?;

    let vaults = rows
        .iter()
        .map(|row| row.get::<i32, _>(0))
        .collect::<Vec<i32>>();
    Ok(vaults)
}

fn build_file_metadata_for_vault(vault: i32, rows: Vec<SqliteRow>) -> Vec<FileMetadata> {
    let mut files = Vec::new();

    rows
        .iter()
        .for_each(|row| {
            let path = PathBuf::from(row.get::<String,_>(1));
            let file = FileMetadata {
                full_path: path,
                root_directory: row.get::<String, _>(2),
                modified_time: row.get::<i64, _>(3),
                file_size: row.get::<i64, _>(4),
                vault_id: vault,
                file_id: row.get::<i32, _>(0),
            };
            files.push(file);
        });
    files
}

/// Used as a helper function to init the DB
fn read_all_local_file_metadata