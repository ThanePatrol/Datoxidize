mod web_server;

use std::fs::File;
use std::path::Path;
use notify::*;
use std::sync::mpsc::channel;
use std::time::{Duration, SystemTime};
use notify::event::{AccessKind, AccessMode};
use std::fs;
use crate::web_server::init_web_server;

fn main() {
    //init webserver
    init_web_server();

    //start channel for passing messages
    let (tx, rx) = channel();

    let mut watcher: Box<dyn Watcher> = if RecommendedWatcher::kind() == WatcherKind::PollWatcher {
        let config = Config::default()
            .with_poll_interval(Duration::from_secs(1));
        Box::new(PollWatcher::new(tx, config).unwrap())
    } else {
        Box::new(RecommendedWatcher::new(tx, Config::default()).unwrap())
    };

    //specify dir to monitor for changes
    watcher.watch(Path::new("./example_dir"), RecursiveMode::Recursive).unwrap();

    // Main event loop, will loop forever and call syncing functions
    for event in rx {
        let e = event.unwrap();

        if let EventKind::Access(AccessKind::Close(AccessMode::Write)) = e.kind {
            //print!("{:?}", e);
            //print!("{:?}", e.paths);
            let start = SystemTime::now();
            sync_file_to_local(e);
            let end = SystemTime::now();
            let time = end.duration_since(start).unwrap();
            println!("{:?}", time);
        }

    }

}

fn sync_file_to_local(event: Event) {
    let file_name = event.paths[0].file_name().unwrap();
    let data = fs::read(event.paths[0].as_path()).expect("");
    let sync_path = "./copy_dir/".to_string() + file_name.to_str().unwrap();
    println!("sync: {}", sync_path);
    fs::write(sync_path, data).expect("Error syncing data");
}

fn sync_file_to_remote(event: Event) {
    let file_name = event.paths[0].file_name().unwrap();
    let data = fs::read(event.paths[0].as_path()).expect("");
    let raw_file_json = F
}

