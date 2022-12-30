use std::path::PathBuf;
use common::RemoteFile;

pub async fn init_sync() {
    let files = get_list_of_files_for_updating();
    let client = reqwest::Client::new();

   let res = reqwest::get("http://localhost:8080")
       .await
       .unwrap()
       .text()
       .await
       .unwrap();
    println!("response is {}", res);

    for file in files {

        let response = client
            .post("http://localhost:8080/copy")
            .json(&file)
            .send()
            .await
            .unwrap();
        println!("{}", response.status())
        //println!("request for {:?} send", file);
    }
}

//todo - make a general syncing
fn get_list_of_files_for_updating() -> Vec<RemoteFile>{
    let mut files = Vec::new();
    let path = PathBuf::from("./client/example_dir");
    let file_paths = common::file_utils::get_all_files_from_path(&path).unwrap();

    for path in file_paths {
        let file = RemoteFile::new(path, "example_dir".to_string(), 0);
        files.push(file);
    }
    files
}

