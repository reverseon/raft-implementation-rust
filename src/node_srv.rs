pub mod helper;
use std::net::{SocketAddr, Ipv4Addr, IpAddr};
use std::sync::{Mutex, Arc};
use std::time::{Instant, Duration};

use helper::config::Config;
use helper::node::Node;
use helper::state::NodeState;
use helper::election::ElectionState;
use helper::rpc::raftrpc::raft_rpc_server::{RaftRpc, RaftRpcServer};
use helper::rpc::raftrpc::{LogEntry, AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse, Empty};
use helper::rpc::raftrpc::raft_rpc_client::RaftRpcClient;
use tonic::{transport::{Server, Channel}, Request, Response, Status};

const CONFIG_PATH: &str = "cfg/config.json";
const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
static mut PORT: u16 = 24341;
static mut NODE_INSTANCE: Option<Node> = None;
static RECEIVED_HEARTBEAT: Mutex<bool> = Mutex::new(false);
static ELECTION_STATE: Mutex<ElectionState> = Mutex::new(ElectionState::Finished);


// HELPER FUNCTION THAT NEEDS DIRECT REFERENCE

fn debug_log<S: AsRef<str>>(msg: S) {
    println!("[{}:{} | {}]: {:?}", HOST, unsafe{PORT}, unsafe {
        NODE_INSTANCE.as_ref().unwrap().state.get_state()
    }, msg.as_ref());
}


// UNSAFE GETTER BECAUSE WE NO MULTITHREAD YO
fn get_instance () -> Node {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().clone()
    }
}
fn get_state () -> NodeState {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().state.clone()
    }
}
fn get_config () -> Config {
    let mut cfg = Config::new();
    cfg.load_from_json(String::from(CONFIG_PATH));
    cfg
}
fn get_port () -> u16 {
    unsafe {
        PORT
    }
}
fn get_log_entries () -> Vec<LogEntry> {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().log.clone()
    }
}
fn get_current_term () -> i32 {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().current_term
    }
}
fn get_voted_for () -> Option<i32> {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().voted_for
    }
}

fn get_last_log_index () -> i32 {
    unsafe {
        (NODE_INSTANCE.as_ref().unwrap().log.len() as i32)-1
    }
}

fn register_node(socket: SocketAddr) {
    let mut cfg = get_config();
    // check if node is already registered
    if cfg.node_address_list.contains(&socket) {
        println!("Node already registered");
        return;
    }
    cfg.node_address_list.push(socket);
    cfg.save_to_json(String::from(CONFIG_PATH));
}

async fn begin_request_voting() {
    println!("Beginning request voting");
    // increment term
    unsafe {
        NODE_INSTANCE.as_mut().unwrap().current_term += 1;
        NODE_INSTANCE.as_mut().unwrap().voted_for = Some(get_port() as i32);
    }
    let mut tally = 1;
    *(ELECTION_STATE.lock().unwrap()) = ElectionState::Running;
    let addresses = get_config().node_address_list;
    let mut new_addresses = Vec::new();
    // generate client except for self
    let mut clients = Vec::new();
    for address in addresses.clone() {
        if address != SocketAddr::new(HOST, get_port()) {
            let cts = match RaftRpcClient::connect(format!("http://{}", address)).await {
                Ok(cts) => {clients.push(cts); new_addresses.push(address);},
                Err(_) => {
                    debug_log(format!("Error connecting to {}", address));
                    continue;
                }
            };
        }
    }
    println!("New addresses: {:?}", new_addresses);
    let mut idx = 0;
    for mut client in clients {
        let request = Request::new(
            RequestVoteRequest {
                term: get_current_term(),
                candidate_id: get_port() as i32,
                last_log_index: get_last_log_index(),
                last_log_term: get_current_term(),
            }
        ); 

        let response = match client.request_vote(request).await {
            Ok(response) => {
                if response.get_ref().vote_granted {
                    tally += 1;
                    println!("Received vote from {}", new_addresses[idx]);
                }
            }
            Err(_) => {
                debug_log(format!("Error sending request to {:?}", new_addresses[idx]));
                continue;
            }
        };
        idx += 1;
    }
    if tally > addresses.len()/2 {
        *(ELECTION_STATE.lock().unwrap()) = ElectionState::Won;
        println!("Won election with {} votes in term {}", tally, get_current_term());
    } else {
        *(ELECTION_STATE.lock().unwrap()) = ElectionState::Lost;
    }
} 

async fn blast_heartbeat() {
    let addresses = get_config().node_address_list;
    // generate client except for self
    let mut clients = Vec::new();
    let mut new_addresses = Vec::new();
    for address in addresses.clone() {
        if address != SocketAddr::new(HOST, get_port()) {
            let cts = match RaftRpcClient::connect(format!("http://{}", address)).await {
                Ok(cts) => {
                    clients.push(cts);
                    new_addresses.push(address);
                },
                Err(_) => {
                    debug_log(format!("Error connecting to {}", address));
                    continue;
                }
            };
        }
    }
    let mut idx = 0;
    for mut client in clients {
        let request = Request::new(
            AppendEntriesRequest {
                term: get_current_term(),
                leader_id: get_port() as i32,
                prev_log_index: get_last_log_index(),
                prev_log_term: get_current_term(),
                entries: Vec::new(),
                leader_commit: 0,
            }
        ); 

        let response = match client.heartbeat(request).await {
            Ok(response) => {
                debug_log(format!("Heartbeat sent to {:?}", new_addresses[idx]));
            }
            Err(e) => {
                // print!("{:?}", e);
                debug_log(format!("Error sending request to {:?}", new_addresses[idx]));
                continue;
            }
        };
        idx += 1;
    }
}

#[derive(Debug, Default)]
pub struct RaftRpcImpl {}

#[tonic::async_trait]
impl RaftRpc for RaftRpcImpl {
    async fn append_entries(
        &self,
        request: tonic::Request<AppendEntriesRequest>,
    ) -> Result<tonic::Response<AppendEntriesResponse>, tonic::Status> {
        // println!("Got a request: {:?}", request);
        let reply = AppendEntriesResponse {
            term: 1,
            success: true,
        };
        Ok(tonic::Response::new(reply))
    }

    async fn request_vote(
        &self,
        request: tonic::Request<RequestVoteRequest>,
    ) -> Result<tonic::Response<RequestVoteResponse>, tonic::Status> {
        println!("Request term: {} with ID: {}", request.get_ref().term, request.get_ref().candidate_id);
        println!("Current term: {}", get_current_term());
        let req = request.into_inner();
        let last_log_index = get_log_entries().len() as i32 - 1;
        if req.term < get_current_term() {
            let reply = RequestVoteResponse {
                term: get_current_term(),
                vote_granted: false,
            };
            return Ok(tonic::Response::new(reply));
        } else if req.term > get_current_term() {
            unsafe {
                NODE_INSTANCE.as_mut().unwrap().current_term = req.term;
                NODE_INSTANCE.as_mut().unwrap().voted_for = Some(req.candidate_id);
            }
            let reply = RequestVoteResponse {
                term: get_current_term(),
                vote_granted: true,
            };
            let mut rh = RECEIVED_HEARTBEAT.lock().unwrap();
            *rh = true;
            println!("Voted for {}", req.candidate_id);
            return Ok(tonic::Response::new(reply));
        } else if (
            get_voted_for().is_none() || get_voted_for() == Some(req.candidate_id)
        ) &&
            helper::election::is_log_left_as_update_as_right(
                req.last_log_term,
                req.last_log_index,
                if last_log_index < 0 { 0 } else { get_log_entries().last().unwrap().term},
                last_log_index,
            )
        {
            let reply = RequestVoteResponse {
                term: get_current_term(),
                vote_granted: true,
            };
            let mut rh = RECEIVED_HEARTBEAT.lock().unwrap();
            *rh = true;
            println!("Voted for {}", req.candidate_id);
            unsafe {
                NODE_INSTANCE.as_mut().unwrap().voted_for = Some(req.candidate_id);
            }
            return Ok(tonic::Response::new(reply));
        } else {
            let reply = RequestVoteResponse {
                term: get_current_term(),
                vote_granted: false,
            };
            return Ok(tonic::Response::new(reply));
        }
    }

    async fn heartbeat(
        &self,
        request: tonic::Request<AppendEntriesRequest>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        // println!("Got a request: {:?}", request);
        if request.get_ref().term < get_current_term() {
            return Ok(tonic::Response::new(Empty {}));
        } else {
            debug_log(format!( "Got a heartbeat from {}",request.get_ref().leader_id ));
            let mut rh = RECEIVED_HEARTBEAT.lock().unwrap();
            *rh = true;
            drop(rh);
            println!("Valid heartbeat from {}", request.get_ref().leader_id);
            Ok(tonic::Response::new(Empty {}))
        }
    } 
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut timekeep: Instant = Instant::now();

    let reset_timer = |timekeep: &mut Instant| {
        *timekeep = Instant::now();
    };

    let has_time_elapsed = |timekeep: &Instant, duration: Duration| -> bool {
        timekeep.elapsed() >= duration
    };

    let args = std::env::args().collect::<Vec<String>>();
    if args.len() > 1 {
        unsafe {
            PORT = args[1].parse::<u16>().unwrap();
        }
    }
    let addr: SocketAddr = SocketAddr::new(HOST, unsafe { PORT });
    unsafe {
        NODE_INSTANCE = Some(Node::new(addr));
    }

    // main loop
    let mut timeout = helper::election::gen_rd_timeout(); 
    reset_timer(&mut timekeep);
    let srv_thread = tokio::task::spawn(
        async move {
            match tonic::transport::Server::builder().add_service(RaftRpcServer::new(RaftRpcImpl::default())).serve(addr).await {
                Ok(_) => {},
                Err(_) => {
                    println!("Error starting server, exiting!");
                    // abort thread
                    return;
                }
            }
        }
    );
    debug_log("Listening...");
    register_node(addr);
    loop {
        if get_state() == NodeState::Follower {
            let mut rh = RECEIVED_HEARTBEAT.lock().unwrap();
            if *rh {
                *rh = false;
                reset_timer(&mut timekeep);
                timeout = helper::election::gen_rd_timeout();
                debug_log("Received heartbeat");
            }
            drop(rh);
            if has_time_elapsed(&timekeep, timeout) {
                unsafe {
                    NODE_INSTANCE.as_mut().unwrap().state = NodeState::Candidate;
                }
                debug_log("Timeout, changing to candidate state");
            } else {
            // print seconds until timeout
                debug_log(&format!("{} seconds until timeout", timeout.as_secs() - timekeep.elapsed().as_secs()));
            }
        } else if get_state() == NodeState::Candidate {
            println!("Election state: {:?}, Received heartbeat {:?}", ELECTION_STATE.lock().unwrap(), RECEIVED_HEARTBEAT.lock().unwrap());
            let mut election_state = ELECTION_STATE.lock().unwrap();
            let mut rh = RECEIVED_HEARTBEAT.lock().unwrap();
            if *rh {
                debug_log("Received heartbeat during election, changing to follower state");
                *election_state = ElectionState::Finished;
                *rh = false;
                reset_timer(&mut timekeep);
                timeout = helper::election::gen_rd_timeout();
                unsafe {
                    NODE_INSTANCE.as_mut().unwrap().state = NodeState::Follower;
                }
            } else if *election_state == ElectionState::Finished {
                debug_log("Sending request for voting");
                // do voting process in new thread
                tokio::task::spawn(
                    async move {
                        begin_request_voting().await;
                    }
                );
            } else if *election_state == ElectionState::Won {
                debug_log("Won election, changing to leader state");
                *election_state = ElectionState::Finished;
                unsafe {
                    NODE_INSTANCE.as_mut().unwrap().state = NodeState::Leader;
                }
            } else if *election_state == ElectionState::Lost {
                debug_log("Lost election, changing to follower state");
                *election_state = ElectionState::Finished;
                reset_timer(&mut timekeep);
                timeout = helper::election::gen_rd_timeout();
                unsafe {
                    NODE_INSTANCE.as_mut().unwrap().state = NodeState::Follower;
                }
            } else {
                debug_log("Election in progress");
            }
        } else if get_state() == NodeState::Leader {
            debug_log("Sending heartbeat");
            blast_heartbeat().await;
            let rh = RECEIVED_HEARTBEAT.lock().unwrap();
            // received a valid heartbeat
            if *rh {
                debug_log("Received heartbeat, changing to follower state");
                unsafe {
                    NODE_INSTANCE.as_mut().unwrap().state = NodeState::Follower;
                }
            }
        }
        // loop every 1 second
        // exit app if server thread is aborted or finished
        if srv_thread.is_finished() {
            debug_log("Server Stopped, exiting!");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }
    Ok(())
}