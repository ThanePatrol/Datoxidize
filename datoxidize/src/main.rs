mod sync_logic;
mod html_creation;

use std::path::Path;
use notify::*;
use std::time::{Duration};
use std::fs;
use crate::sync_logic::{create_folder_on_remote, deserialize_config, sync_changed_file};
use axum::{
    routing::{get, post},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use notify::event::CreateKind::Folder;
use notify::EventKind::Create;
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<()> {


    let dir_settings = deserialize_config("./test_resources/config.json".to_string()).unwrap();
    let watched_dir = &dir_settings.content_directory.clone();

    sync_logic::initial_sync(&dir_settings);

    //todo - create a atomic boolean value to limit writes in short succession
    //todo - use a timer to determine when file was last modified
    //todo - if there is another event before this timer reaches 0, reset the timer
    //todo - so if a file is edited many times it will only be synced after a period of inactivity

    //NB - This watcher needs to be initiated like this to allow for asynchronous runtime be
    //between web server and notifications
    let mut watcher =
        RecommendedWatcher::new(move |result: Result<Event>| {
            let event = result.unwrap();

            if event.kind.is_modify() {
                sync_changed_file(&event.paths, &dir_settings);
            } else if event.kind.is_create()  && event.kind == Create(Folder) {
                create_folder_on_remote(&event.paths, &dir_settings);
            } else if event.kind.is_remove() {
                sync_logic::remove_files_and_dirs_from_remote(&event.paths, &dir_settings);
            }
        },notify::Config::default()
                                    .with_poll_interval(Duration::from_secs(1)))?;

    watcher.watch(Path::new(watched_dir.as_str()), RecursiveMode::Recursive)?;
    //todo - set duration::from_secs() from user preferences.

    //html_creation::test_print_html();



    Ok(())
}

