#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Config {
    pub node_address_list: Vec<std::net::SocketAddr>,
}


impl Config {
    pub fn new() -> Self {
        Config {
            node_address_list: Vec::new(),
        }
    }

    pub fn load_from_json(&mut self, json_filename: String) {
        let json_string = super::rw::read_file_create_file_and_dir_if_not_exist(json_filename.clone());
        if json_string == "" {
            // initialize
            self.node_address_list = Vec::new();
            self.save_to_json(json_filename);
        } else {
            let json: serde_json::Value = serde_json::from_str(&json_string).expect("Error parsing JSON file");
            let node_address_list: Vec<std::net::SocketAddr> = serde_json::from_value(json["node_address_list"].clone()).expect("Error parsing JSON file");
            self.node_address_list = node_address_list;
        }
    }

    pub fn save_to_json(&self, json_filename: String) {
        let json_string = serde_json::to_string(&self).expect("Error serializing Config to JSON");
        super::rw::write_file_create_file_and_dir_if_not_exist(json_filename, json_string);
    }
}