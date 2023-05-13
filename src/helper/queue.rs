use super::rpc::raftrpc::LogEntry;

pub struct Queue {
    pub queue: Vec<String>,
}

impl Queue {
    pub fn new() -> Self {
        Queue { queue: Vec::new() }
    }

    pub fn enqueue(&mut self, item: String) {
        self.queue.push(item);
    }

    pub fn dequeue(&mut self) -> Option<String> {
        self.queue.pop()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn build_from_log_entries(&mut self, log_entries: &Vec<LogEntry>) {
        for log_entry in log_entries {
            let cmd: String = log_entry.command.clone();
            // if cmd begins with ENQUEUE, then enqueue the item
            if cmd.starts_with("ENQUEUE") {
                let item: String = cmd.split(" ").collect::<Vec<&str>>()[1].to_string();
                self.enqueue(item);
            }
            // if cmd begins with DEQUEUE, then dequeue the item
            else if cmd.starts_with("DEQUEUE") {
                self.dequeue();
            }
        }
    }

    pub fn enqueue_log(item: &str) -> String {
        format!("ENQUEUE {}", item)
    }

    pub fn dequeue_log() -> String {
        String::from("DEQUEUE")
    }

}