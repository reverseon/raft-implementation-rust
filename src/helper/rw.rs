pub fn write_file_create_file_and_dir_if_not_exist(filename: String, content: String) {
    // get directory name from filename
    let dir_name = std::path::Path::new(&filename).parent().unwrap().to_str().unwrap();
    match std::fs::write(filename.clone(), content.clone()) {
        Ok(_) => {},
        // if the directory doesn't exist, create it
        Err(_) => {
            std::fs::create_dir_all(dir_name).expect("Error creating directory");
            match std::fs::write(filename, content) {
                Ok(_) => {},
                Err(_) => panic!("Error writing file")
            }
        }
    }
}

pub fn read_file_create_file_and_dir_if_not_exist(filename: String) -> String {
    match std::fs::read_to_string(filename.clone()) {
        Ok(content) => content,
        // if the file doesn't exist, create it and write an empty string
        Err(_) => {
            write_file_create_file_and_dir_if_not_exist(filename.clone(), String::from(""));
            String::from("")
        }
    }
}