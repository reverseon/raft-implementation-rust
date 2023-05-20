use super::rpc::raftrpc::LogEntry;

pub struct Queue {
    pub queue: Vec<String>,
}

impl Queue {
    pub fn new() -> Self {
        Queue { queue: Vec::new() }
    }

    fn enqueue(&mut self, item: String) {
        self.queue.push(item);
    }

    fn dequeue(&mut self) -> Option<String> {
        self.queue.pop()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn build_from_log_entries(&mut self, log_entries: &Vec<LogEntry>) {
        for log_entry in log_entries {
            let cmd: String = log_entry.command.clone();
            self.execute(cmd.to_string()); 
        }
    }

    pub fn peekall_log(&self) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        for item in self.queue.clone() {
            result.push(item);
        }
        result
    }

    pub fn enqueue_log(item: &str) -> String {
        format!("ENQUEUE {}", item)
    }

    pub fn dequeue_log() -> String {
        String::from("DEQUEUE")
    }

    pub fn execute(&mut self, cmd: String) -> String {
        let mut result: String = String::new();
        if cmd.starts_with("ENQUEUE") {
            let item: String = cmd.split(" ").collect::<Vec<&str>>()[1].to_string();
            self.enqueue(item);
            result = format!("");
        } else if cmd.starts_with("DEQUEUE") {
            let item: Option<String> = self.dequeue();
            match item {
                Some(item) => {
                    result = format!("{}", item);
                }
                None => {
                    result = String::from("QUEUE EMPTY");
                }
            }
        }
        result
    }

}