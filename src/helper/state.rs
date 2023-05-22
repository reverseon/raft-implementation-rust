
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NodeState {
    Candidate,
    Follower,
    Leader,
}

impl NodeState {
    pub fn get_state(&self) -> &str {
        match self {
            NodeState::Candidate => "Candidate",
            NodeState::Follower => "Follower",
            NodeState::Leader => "Leader",
        }
    }
}

impl Default for NodeState {
    fn default() -> Self {
        NodeState::Follower
    }
}
