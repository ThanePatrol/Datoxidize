use std::path::Path;
use notify::*;
use std::sync::mpsc::channel;
use std::time::Duration;

fn main() {
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

    for event in rx {
        println!("{:?}", event);
    }

}
