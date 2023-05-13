use super::rpc::raftrpc::LogEntry;
use super::state::NodeState;
use serde::ser::{
    SerializeStruct
};


#[derive(serde::Serialize, serde::Deserialize)]
pub struct LogEntryWritable {
    pub term: i32,
    pub index: i32,
    pub command: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Persistent {
    pub current_term: i32,
    pub voted_for: Option<i32>,
    pub log: Vec<LogEntryWritable>,
}

impl Persistent {
   pub fn log_entry_to_writable(log_entry: &Vec<LogEntry>) -> Vec<LogEntryWritable> {
        let mut log_entry_writable: Vec<LogEntryWritable> = Vec::new();
        for entry in log_entry {
            let log_entry_writable_item = LogEntryWritable {
                term: entry.term as i32,
                index: entry.index as i32,
                command: entry.command.clone(),
            };
            log_entry_writable.push(log_entry_writable_item);
        }
        log_entry_writable
    }
    pub fn writable_to_log_entry(writable: &Vec<LogEntryWritable>) -> Vec<LogEntry> {
        let mut log_entry: Vec<LogEntry> = Vec::new();
        for entry in writable {
            let log_entry_item = LogEntry {
                term: entry.term as i32,
                index: entry.index as i32,
                command: entry.command.clone(),
            };
            log_entry.push(log_entry_item);
        }
        log_entry
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub address: std::net::SocketAddr,
    pub current_term: i32,
    pub voted_for: Option<i32>,
    pub log: Vec<LogEntry>,
    pub commit_index: i32,
    pub last_applied: i32,
    pub next_index: Option<Vec<i32>>,
    pub match_index: Option<Vec<i32>>,
    pub state: NodeState
}

impl Node {
    pub fn new(address: std::net::SocketAddr) -> Self {
        Node {
            address,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            next_index: None,
            match_index: None,
            state: NodeState::Follower
        }
    }
    pub fn save_persistent_to_json(&self) {
        let json_filename = format!("logs/node_{}.json", self.address);
        let persistent = Persistent {
            current_term: self.current_term,
            voted_for: self.voted_for,
            log: Persistent::log_entry_to_writable(&self.log),
        };
        let json_string = serde_json::to_string(&persistent).expect("Error serializing Node to JSON");
        super::rw::write_file_create_file_and_dir_if_not_exist(json_filename, json_string);
    }

    pub fn load_persistent_from_json(&mut self) {
        let json_filename = format!("logs/node_{}.json", self.address);
        let json_string = super::rw::read_file_create_file_and_dir_if_not_exist(json_filename.clone()); 
        if json_string == "" {
            // initialize
            self.current_term = 0;
            self.voted_for = None;
            self.log = Vec::new();
            self.save_persistent_to_json();
        } else {
            let persistent: Persistent = serde_json::from_str(&json_string).expect("Error parsing JSON file");
            self.current_term = persistent.current_term;
            self.voted_for = persistent.voted_for;
            self.log = Persistent::writable_to_log_entry(&persistent.log);
        }
    }
}
