pub mod helper;

// REFERENCE
// https://www.youtube.com/watch?v=uXEYuDwm7e4

use std::cmp::{min, max};
use std::future::{IntoFuture, Future};
use std::net::{SocketAddr, Ipv4Addr, IpAddr};
use std::sync::{Arc};
use tokio::sync::{RwLock};
use tonic::codegen::http::response;
use std::time::{Instant, Duration};
use std::collections::{HashSet, HashMap, VecDeque};
use rand::Rng;


use helper::config::Config;
use helper::node::{Node, self};
use helper::timer::{
    RandomizedTimer,
    get_randomized_duration,
};
use helper::state::NodeState;
use helper::election::ElectionState;
use helper::rpc::raftrpc::raft_rpc_server::{RaftRpc, RaftRpcServer};
use helper::rpc::raftrpc::{
    LogEntry, AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, 
    RequestVoteResponse, Empty, JoinRequest, JoinResponse, UpdateConfigRequest, UpdateConfigResponse, 
    EnqueueRequest, EnqueueResponse,
    DequeueRequest, DequeueResponse,
    ReadQueueRequest, ReadQueueResponse,
};
use helper::rpc::raftrpc::raft_rpc_client::RaftRpcClient;
use tonic::{transport::{Server, Channel, Endpoint}, Request, Response, Status};

const CONFIG_PATH: &str = "cfg/config.json";
static mut CONFIG : Option<RwLock<Config>> = None;
const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
static mut PORT: u16 = 24341;
static mut NODE_INSTANCE: Option<RwLock<Node>> = None;
static mut HEARTBEAT_TIMER: Option<RwLock<RandomizedTimer>> = None;
static mut STATE_MACHINE: Option<RwLock<helper::queue::Queue>> = None;
// static mut ELECTION_TIMER: Option<RwLock<RandomizedTimer>> = None;
const LOWER_CEIL_MS: u64 = 150;
const UPPER_CEIL_MS: u64 = 300;
const HEARTBEAT_MS: u64 = 100;


// HELPER FUNCTION THAT NEEDS DIRECT REFERENCE

async fn print_status<S: AsRef<str>>(msg: S) {
    // last 4 digits of millis
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let millis = millis - (millis / 10000) * 10000;
    println!("[{:04} | {}:{} | {} | Term {}]: {}", millis, HOST, unsafe{PORT}, unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.state.get_state()
    }, get_current_term().await, msg.as_ref());
}

async fn get_leader_address () -> Option<SocketAddr> {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.leader_address
    }
}

async fn set_leader_address (addr: Option<SocketAddr>) {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().write().await.leader_address = addr;
    }
}


// UNSAFE GETTER BECAUSE WE NO MULTITHREAD YO
async fn get_state () -> NodeState {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.state.clone()
    }
}

async fn load_config () {
    print_status("Loading config").await;
    let mut cfg = Config::new();
    cfg.load_from_json(String::from(CONFIG_PATH));
    unsafe {
        CONFIG = Some(RwLock::new(cfg));
    }
}

async fn get_config () -> Config {
    // let mut cfg = Config::new();
    // cfg.load_from_json(String::from(CONFIG_PATH));
    // cfg
    unsafe  {
        CONFIG.as_ref().unwrap().read().await.clone()
    }
}

async fn insert_to_config (socket: SocketAddr) {
    unsafe {
        CONFIG.as_mut().unwrap().write().await.node_address_list.insert(socket);
    }
}

async fn save_config () {
    print_status("Saving config").await;
    unsafe {
        CONFIG.as_ref().unwrap().write().await.save_to_json(String::from(CONFIG_PATH));
    }
}

async fn is_in_config(socket: SocketAddr) -> bool {
    unsafe {
        CONFIG.as_ref().unwrap().read().await.node_address_list.contains(&socket)
    }
}

fn get_port () -> u16 {
    unsafe {
        PORT
    }
}
async fn get_log_entries () -> Vec<LogEntry> {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.log.clone()
    }
}
async fn get_log_entry (index: usize) -> LogEntry {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.log[index].clone()
    }
}

async fn set_log_entry (index: usize, entry: LogEntry) {
    unsafe {
        let mut locknode = NODE_INSTANCE.as_mut().unwrap().write().await;
        locknode.log[index] = entry;
        locknode.save_persistent_to_json();
    }
}

async fn append_to_log (entry: LogEntry) {
    unsafe {
        let mut locknode = NODE_INSTANCE.as_mut().unwrap().write().await;
        locknode.log.push(entry); 
        locknode.save_persistent_to_json();
    }
}

async fn set_log_entries (entries: Vec<LogEntry>) {
    unsafe {
        let mut locknode = NODE_INSTANCE.as_mut().unwrap().write().await;
        locknode.log = entries;
        locknode.save_persistent_to_json();
    }
}

async fn get_current_term () -> i32 {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.current_term
    }
}

async fn set_current_term (term: i32) {
    unsafe {
        let mut nodeinstlock = NODE_INSTANCE.as_mut().unwrap().write().await;
        nodeinstlock.current_term = term;
        nodeinstlock.save_persistent_to_json();
    }
}

async fn get_voted_for () -> Option<i32> {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.voted_for
    }
}

async fn set_voted_for (voted_for: Option<i32>) {
    unsafe {
        let mut nodeinstlock = NODE_INSTANCE.as_mut().unwrap().write().await;
        nodeinstlock.voted_for = voted_for;
        nodeinstlock.save_persistent_to_json();
    }
}

async fn get_last_log_index () -> i32 {
    unsafe {
        let loglen = NODE_INSTANCE.as_ref().unwrap().read().await.log.len();
        if loglen <= 0 {
            return 0;
        } else {
            return loglen as i32 - 1;
        }
    }
}

async fn get_log_len () -> i32 {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.log.len() as i32
    }
}

// async fn reset_election_timer () {
//     unsafe {
//     let mut timer = ELECTION_TIMER.as_ref().unwrap().write().await;
//     timer.reset();
//     }
// }

async fn reset_heartbeat_timer () {
    unsafe {
    let mut timer = HEARTBEAT_TIMER.as_mut().unwrap().write().await;
    timer.reset();
    }
}

// async fn is_election_timer_expired () -> bool {
//     unsafe {
//     let timer = ELECTION_TIMER.as_mut().unwrap().read().await;
//     timer.is_expired()
//     }
// }

async fn is_heartbeat_timer_expired () -> bool {
    unsafe {
    let timer = HEARTBEAT_TIMER.as_ref().unwrap().read().await;
    timer.is_expired()
    }
}

async fn get_match_index (socket: SocketAddr) -> i32 {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.match_index.get(&socket).unwrap().clone()
    }
}

async fn is_match_index_existed (socket: SocketAddr) -> bool {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.match_index.contains_key(&socket)
    }
}

async fn get_next_index (socket: SocketAddr) -> i32 {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.next_index.get(&socket).unwrap().clone()
    }
}

async fn is_next_index_existed (socket: SocketAddr) -> bool {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.next_index.contains_key(&socket)
    }
}

async fn set_match_index (socket: SocketAddr, index: i32) {
    unsafe {
        let mut nodeinstlock = NODE_INSTANCE.as_ref().unwrap().write().await;
        nodeinstlock.match_index.insert(socket, index);
    }
}

async fn set_next_index (socket: SocketAddr, index: i32) {
    unsafe {
        let mut nodeinstlock = NODE_INSTANCE.as_ref().unwrap().write().await;
        nodeinstlock.next_index.insert(socket, index);
    }
}

async fn get_sockets_from_config () -> Vec<SocketAddr> {
    unsafe {
        CONFIG.as_ref().unwrap().read().await.node_address_list.iter().cloned().collect::<Vec<SocketAddr>>()
    }
}

async fn get_commit_index () -> i32 {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.commit_index
    }
}

async fn set_commit_index (index: i32) {
    unsafe {
        let mut nodeinstlock = NODE_INSTANCE.as_mut().unwrap().write().await;
        nodeinstlock.commit_index = index;
    }
}

async fn apply_command_to_state_machine (command: String) -> String {
    unsafe {
        STATE_MACHINE.as_mut().unwrap().write().await.execute(command)
    }
}

async fn get_channel(socket: SocketAddr) -> Option<RaftRpcClient<Channel>> {
    match tokio::time::timeout(Duration::from_millis(HEARTBEAT_MS/5), 
        RaftRpcClient::connect(format!("http://{}", socket.to_string()))
    ).await {
        Ok(channel) => {
            match channel {
                Ok(channel) => Some(channel),
                Err(_) => None
            }
        }
        Err(_) => None
    }
}

async fn get_random_duration() -> Duration {
    let mut rng = rand::thread_rng();
    let random = rng.gen_range(LOWER_CEIL_MS..UPPER_CEIL_MS);
    Duration::from_millis(random)
}

async fn acks(length: i32) -> i32 {
    let mut counter = 1;
    for socket in get_sockets_from_config().await {
        // println!("checking ack from {}", socket.to_string());
        if socket == SocketAddr::new(HOST, get_port()) {
            continue;
        } else if get_match_index(socket).await + 1 >= length {
            // println!("ack from {}", socket.to_string());
            counter += 1;
        }
    }
    return counter;
}


async fn replicate_log_to_all_followers() ->  String {        
    if get_state().await != NodeState::Leader {
        return "Not leader".to_string();
    }
    let mut threadset = tokio::task::JoinSet::new();
    for socket in get_sockets_from_config().await {
        if socket == SocketAddr::new(HOST, get_port()) {
            continue;
        }
        threadset.spawn( async move {
            let mut channel = match get_channel(socket).await {
                Some(channel) => channel,
                None => {
                    return;
                }
            };
            loop { 
                let req = craft_append_entries_request(socket).await;
                // println!("request: {:?}", req.get_ref());
                // println!("socket: {}", socket.to_string());
                match channel.append_entries(req).await {
                    Ok(response) => {
                        let response = response.into_inner();
                        let success = response.success;
                        let term = response.term;
                        let last_applied_index = response.last_applied_index;
                        if term == get_current_term().await && get_state().await == NodeState::Leader {
                            if success && last_applied_index > get_match_index(socket).await {
                                set_match_index(socket, last_applied_index).await;
                                set_next_index(socket, last_applied_index+1).await;
                                
                                return;
                            } else if get_next_index(socket).await > 0 {
                                set_next_index(socket, get_next_index(socket).await-1).await;
                                // continue looping
                            } else {
                                return;
                            }
                        } else if term > get_current_term().await {
                            set_current_term(term).await;
                            change_state_to(NodeState::Follower, "Receive higher term").await;
                            set_voted_for(None).await;
                            reset_heartbeat_timer().await;
                            return;
                        } else {
                            return;   
                        }
                    }
                    Err(_) => {
                        return;
                    }
                }
            }
            
        });
    }
    while let Some(_) = threadset.join_next().await {}
    // CommitLogEntries
    let majoritytally = get_sockets_from_config().await.len() / 2;
    let mut readyvec = Vec::new();
    // println!("");
    for len in 1..get_log_len().await+1 {
        // println!("checking acks for {}", len);
        if acks(len).await > majoritytally as i32 {
            // println!("up to {} committed", len);
            readyvec.push(len);
        }
    }
    // println!("");
    let mut last_result = "No new entries to commit".to_string();
    if readyvec.len() > 0 {
        let maxrvec = *(readyvec.iter().max().unwrap());
        // println!("log term: {}", get_log_entry(maxrvec as usize - 1).await.term);
        // println!("current term: {}", get_current_term().await);
        if maxrvec > get_commit_index().await + 1 && get_log_entry(maxrvec as usize - 1).await.term <= get_current_term().await {
            for i in get_commit_index().await+1..maxrvec {
                let command = get_log_entry(i as usize).await.command.clone();
                last_result = apply_command_to_state_machine(command).await;
            }
            set_commit_index(maxrvec - 1).await;
        }
    }
    return last_result;
}

async fn do_append_entries(prev_log_index: i32, leader_commit_index: i32, to_append: Vec<LogEntry>) {
    // println!("");
    // println!("Append entries with prev_log_index: {}, leader_commit_index: {}, my_commit_index: {}, to_append: {:?}", prev_log_index, leader_commit_index, get_commit_index().await, to_append);
    // println!("");
    let to_append_len: i32 = to_append.len() as i32;
    if to_append_len > 0 && get_log_len().await > prev_log_index+1 {
        let index: usize = (min(
            get_log_len().await,
            prev_log_index+1+to_append_len
        ) - 1) as usize;
        if get_log_entry(index).await.term != to_append[((index as i32)-prev_log_index-1) as usize].term {
            unsafe {
                let mut nodeinstlock = NODE_INSTANCE.as_mut().unwrap().write().await;
                nodeinstlock.log.truncate((prev_log_index+1) as usize);
                nodeinstlock.save_persistent_to_json();
                drop(nodeinstlock);
            }
        }
    }
    if prev_log_index+1+to_append_len > get_log_len().await {
        for i in (get_log_len().await-prev_log_index-1)..to_append_len {
            append_to_log(to_append[i as usize].clone()).await;
        }
    }
    if leader_commit_index > get_commit_index().await {
        for i in get_commit_index().await+1..leader_commit_index+1 {
            apply_command_to_state_machine(get_log_entry(i as usize).await.command.clone()).await; 
        }
        set_commit_index(leader_commit_index).await;
    }
}


async fn begin_request_voting() {
    // if config is <= 1, immediately become leader
    if get_config().await.node_address_list.len() <= 1 {
        for socket in get_sockets_from_config().await {
            if socket == SocketAddr::new(HOST, get_port()) {
                continue;
            }
            set_next_index(socket, get_log_len().await).await;
            set_match_index(socket, -1).await;
        }
        change_state_to(NodeState::Leader, format!("Won election")).await;
        return;
    }
    // update_client_pool().await;
    let loglen = get_log_len().await;
    let last_log_term = if loglen > 0 {
        get_log_entry(loglen as usize - 1).await.term
    } else {
        0
    };
    // send request vote to all nodes in different threads until conclusive
    while get_state().await == NodeState::Candidate {
        let sockets = get_sockets_from_config().await.iter().cloned().collect::<Vec<SocketAddr>>();
        print_status(format!("Begin election with term {}", get_current_term().await)).await;
        let voted_ids = Arc::new(RwLock::new(HashSet::<i32>::new()));
        let mut vidw = voted_ids.write().await;
        vidw.insert(get_port() as i32);
        drop(vidw); // release lock
        let majoritytally = sockets.len() / 2;
        let electdur = get_random_duration().await;
        let mut threadset = tokio::task::JoinSet::new();
        for socket in sockets {
            if socket == SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), get_port()) {
                continue;
            }
            let voted_ids_clone = Arc::clone(&voted_ids);
            threadset.spawn(async move {
                let mut client = match get_channel(socket).await {
                    Some(client) => {
                        // println!("Connected to {}", socket);
                        client
                    },
                    None => {
                        println!("Failed to connect to {}", socket);
                        return;
                    }
                };
                let request = Request::new(
                    RequestVoteRequest {
                        term: get_current_term().await,
                        candidate_id: get_port() as i32,
                        last_log_index: get_last_log_index().await,
                        last_log_term: last_log_term,
                        candidate_address: format!("{}", SocketAddr::new(HOST, get_port()))
                    }
                ); 
                match tokio::time::timeout(electdur, client.request_vote(request)).await {
                    Ok(rp) => {
                        match rp {
                            Ok(response) => {
                                if get_state().await == NodeState::Candidate && response.get_ref().term == get_current_term().await && response.get_ref().vote_granted {
                                    let mut vidcw = voted_ids_clone.write().await;
                                    vidcw.insert(socket.port() as i32);
                                    drop(vidcw);
                                    let votedidlock = voted_ids_clone.read().await;
                                    let votedidlen = votedidlock.len();
                                    drop(votedidlock);
                                    if votedidlen > majoritytally {
                                        for socket in get_sockets_from_config().await {
                                            if socket == SocketAddr::new(HOST, get_port()) {
                                                continue;
                                            }
                                            set_next_index(socket, get_log_len().await).await;
                                            set_match_index(socket, -1).await;
                                        }
                                        change_state_to(NodeState::Leader, format!("Won election")).await;
                                    } 
                                    print_status(format!("Received vote from {}", socket)).await;
                                } else if response.get_ref().term > get_current_term().await {
                                    change_state_to(NodeState::Follower, format!("Received higher term from {}", socket)).await;
                                    set_leader_address(Some(socket)).await;
                                    reset_heartbeat_timer().await;
                                    set_current_term(response.get_ref().term).await;
                                    set_voted_for(None).await;
                                }
                            },
                            Err(_) => {
                                print_status(format!("Error receiving response from {}", socket)).await;
                            }
                        }
                    },
                    Err(_) => {
                        print_status(format!("Error sending request to {}", socket)).await;
                    }
                };
            });
        }
        while let Some(_) = threadset.join_next().await {
            // wait for all threads to finish
        }
        if get_state().await != NodeState::Candidate {
            let voted_ids_lock = voted_ids.read().await;
            let printedval = voted_ids_lock.iter().map(|x| x.to_string()).collect::<Vec<String>>();
            drop(voted_ids_lock);
            print_status(
                format!("Election ended with vote: {:?} at term: {}", printedval, get_current_term().await)
            ).await;
        } else /* if timekeep.elapsed().as_millis() > electdur.as_millis() */ {            
            let voted_ids_lock = voted_ids.read().await;
            let printedval = voted_ids_lock.iter().map(|x| x.to_string()).collect::<Vec<String>>();
            drop(voted_ids_lock);
            print_status("No winner, resetting election").await;        
            print_status(
                format!("Election ended with vote: {:?} at term: {}", printedval, get_current_term().await)
            ).await;
            set_current_term(get_current_term().await + 1).await;
            set_voted_for(Some(get_port() as i32)).await;
            // abort all handlers
        }
    }
}

async fn get_last_applied() -> i32 {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.last_applied
    }
}

async fn set_last_applied(last_applied: i32) {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().write().await.last_applied = last_applied;
    }
}
    

// async fn blast_heartbeat() {
//     if get_state().await != NodeState::Leader {
//         return;
//     }
//     // send heartbeat to all nodes in different threads
//     // let addresses = get_config().node_address_list;
//     // let mut clients = Vec::new();
//     // let mut new_addresses = Vec::new();
//     // for address in addresses.clone() {
//     //     if address != SocketAddr::new(HOST, get_port()) {
//     //         let cts = match RaftRpcClient::connect(format!("http://{}", address)).await {
//     //             Ok(cts) => {
//     //                 clients.push(cts);
//     //                 new_addresses.push(address);
//     //             },
//     //             Err(_) => {
//     //                 print_status(format!("Error connecting to {}", address));
//     //                 continue;
//     //             }
//     //         };
//     //     }
//     // }
//     // update_client_pool().await;
//     let socketsclone = get_sockets_from_config().await.iter().cloned().collect::<Vec<SocketAddr>>();
//     for socket in socketsclone { 
//         if socket == SocketAddr::new(HOST, get_port()) {
//             continue;
//         }
//         // print_status(format!("Sending heartbeat to {}", socket)).await;
//         tokio::spawn( async move {
//             let request = Request::new(
//                 AppendEntriesRequest {
//                     term: get_current_term().await,
//                     leader_id: get_port() as i32,
//                     prev_log_index: get_last_log_index().await,
//                     prev_log_term: get_current_term().await,
//                     entries: Vec::new(),
//                     leader_commit: 0,
//                     leader_address: format!("{}", SocketAddr::new(HOST, get_port())),
//                 }
//             ); 
//             let mut channel = match get_channel(socket as SocketAddr).await {
//                 Some(channel) => channel,
//                 None => {
//                     // print_status(format!("Error connecting to {}", socket)).await;
//                     return;
//                 }
//             };
//             match channel.heartbeat(request).await {
//                 Ok(_) => {
//                 }
//                 Err(_) => {
//                 }
//             };
//         });
//     }

// }

async fn change_state_to<S: AsRef<str>>(state: NodeState, reason: S) {
    print_status(format!("Changing state to {} because {}", state.get_state(), reason.as_ref())).await;
    if state == NodeState::Leader {
        set_leader_address(
            Some(
                SocketAddr::new(HOST, get_port())
            )
        ).await;
    } else if state == NodeState::Candidate {
        set_leader_address(None).await;
    }
    unsafe {
        NODE_INSTANCE.as_mut().unwrap().write().await.state = state;
    }
}

async fn craft_append_entries_request(dest: SocketAddr) -> Request<AppendEntriesRequest> {
    if get_state().await != NodeState::Leader {
        return Request::new(
            AppendEntriesRequest {
                term: -1, // ignore this
                leader_id: get_port() as i32,
                prev_log_index: get_last_log_index().await,
                prev_log_term: get_current_term().await,
                entries: Vec::new(),
                leader_commit: 0,
                leader_address: format!("{}", SocketAddr::new(HOST, get_port())),
            }
        );
    } else {
        let next_index = get_next_index(dest).await;
        let mut entries: Vec<LogEntry> = Vec::new();
        for i in next_index..get_log_len().await {
            entries.push(get_log_entry(i as usize).await);
        }
        let prev_log_index = next_index - 1;
        let prev_log_term = if next_index > 0 {get_log_entry(prev_log_index as usize).await.term} else {0};
        Request::new(
            AppendEntriesRequest {
                term: get_current_term().await,
                leader_id: get_port() as i32,
                prev_log_index: prev_log_index,
                prev_log_term: prev_log_term,
                entries,
                leader_commit: get_commit_index().await,
                leader_address: format!("{}", SocketAddr::new(HOST, get_port())),
            }
        )
    }
}

async fn peek_all_queue() -> Vec<String> {
    unsafe {
        STATE_MACHINE.as_ref().unwrap().read().await.peekall_log()
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
        let req_term = request.get_ref().term;
        // let req_leader_id = request.get_ref().leader_id;
        let req_prev_log_index = request.get_ref().prev_log_index;
        let req_prev_log_term = request.get_ref().prev_log_term;
        let req_entries = request.get_ref().clone().entries;
        let req_leader_commit = request.get_ref().leader_commit;
        let req_leader_address = request.get_ref().clone().leader_address;
        if req_term > get_current_term().await {
            set_current_term(req_term).await;
            set_voted_for(None).await;
        }
        if req_term == get_current_term().await {
            // println!("Got a valid append entries request from {}", req_leader_address);
            if get_state().await != NodeState::Follower {
                change_state_to(NodeState::Follower, "got valid append entries request").await;
            }
            set_leader_address(Some(req_leader_address.parse().unwrap())).await;
            reset_heartbeat_timer().await;
        }
        let logvalid = get_log_len().await >= req_prev_log_index + 1 && 
        (req_prev_log_index == -1 || get_log_entry(req_prev_log_index.try_into().unwrap()).await.term == req_prev_log_term);
        if req_term == get_current_term().await && logvalid {
            do_append_entries(req_prev_log_index, req_leader_commit, req_entries.clone()).await;
            set_last_applied(req_prev_log_index /* + 1 */ + req_entries.len() as i32 /* -1 */).await; 
            // println!("Last applied: {}", get_last_applied().await);
            return Ok(Response::new(
                AppendEntriesResponse {
                    term: get_current_term().await,
                    success: true,
                    last_applied_index: get_last_applied().await,
                }
            ));

        } else {
            return Ok(Response::new(
                AppendEntriesResponse {
                    term: get_current_term().await,
                    success: false,
                    last_applied_index: 0,
                }
            ));
        }
    }

    async fn request_vote(
        &self,
        request: tonic::Request<RequestVoteRequest>,
    ) -> Result<tonic::Response<RequestVoteResponse>, tonic::Status> {
        print_status(format!("Received request vote from {} with Term: {} in my current Term: {}", 
        request.get_ref().candidate_id,
        request.get_ref().term,
        get_current_term().await)).await;
        let req = request.into_inner();
        let candidate_id = req.candidate_id;
        let candidate_term = req.term;
        let candidate_last_log_index = req.last_log_index;
        let candidate_last_log_term = req.last_log_term;
        let candidate_socket: SocketAddr = req.candidate_address.parse().unwrap(); 
        if candidate_term > get_current_term().await {
            set_current_term(candidate_term).await;
            change_state_to(NodeState::Follower, "Received request vote from a higher term").await;
            set_leader_address(Some(candidate_socket)).await;
            reset_heartbeat_timer().await;
            set_voted_for(None).await;
        }
        let last_log_term = if get_log_len().await > 0 {
            get_log_entry(get_last_log_index().await as usize).await.term
        } else {
            0
        };
        let is_log_up_to_date = (candidate_last_log_term > last_log_term) 
        || (candidate_last_log_term == last_log_term && candidate_last_log_index >= get_last_log_index().await);
        // debug
        if candidate_term == get_current_term().await 
        && (get_voted_for().await.is_none() || get_voted_for().await.unwrap() == candidate_id) 
        && is_log_up_to_date {
            set_voted_for(Some(candidate_id)).await;
            let reply = RequestVoteResponse {
                term: get_current_term().await,
                vote_granted: true,
            };
            print_status(format!("Voted for {}", candidate_id)).await;
            Ok(tonic::Response::new(reply))
        } else {
            let reply = RequestVoteResponse {
                term: get_current_term().await,
                vote_granted: false,
            };
            Ok(tonic::Response::new(reply))
        }
    }

    async fn heartbeat(
        &self,
        request: tonic::Request<AppendEntriesRequest>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        // println!("Got a request: {:?}", request);
        if request.get_ref().term < get_current_term().await {
            return Ok(tonic::Response::new(Empty {}));
        } else {
            // print_status(format!( "Got a heartbeat from {}",request.get_ref().leader_id )).await;
            // if not in follower state, change to follower                change_state_to(NodeState::Follower, "Received heartbeat from leader").await;
            let leader_socket: SocketAddr = request.get_ref().leader_address.parse().unwrap();
            set_leader_address(Some(leader_socket)).await;
            if get_state().await != NodeState::Follower {
                change_state_to(NodeState::Follower, "Received heartbeat from leader").await;
            }
            // update term
            set_current_term(request.get_ref().term).await;
            reset_heartbeat_timer().await;
            Ok(tonic::Response::new(Empty {}))
        }
    } 

    async fn join(
        &self,
        request: tonic::Request<JoinRequest>,
    ) -> Result<tonic::Response<JoinResponse>, tonic::Status> {
        let socket: SocketAddr = request.get_ref().address.parse().unwrap(); 
        if get_state().await == NodeState::Leader {
            if !is_in_config(socket).await {
                insert_to_config(socket).await;
                save_config().await;

            }
            if !is_match_index_existed(socket).await {          
                set_match_index(socket, -1).await;
            }
            if !is_next_index_existed(socket).await {
                set_next_index(socket, get_log_len().await).await;
            }
            let reply = JoinResponse {
                success: true,
            };
            // broadcast to all nodes except self to update
            let sockets = get_sockets_from_config().await;
            for socket in sockets {
                if socket != SocketAddr::new(HOST, get_port()) {
                    let mut client = match get_channel(socket).await {
                        Some(client) => client,
                        None => continue,
                    };
                    for _ in 0..3 {                    
                        let mut addresses: Vec<String> = Vec::new();
                        for address in get_config().await.node_address_list.iter() {
                            addresses.push(address.to_string());
                        }
                        let request = tonic::Request::new(UpdateConfigRequest {
                            addresses
                        });
                        match tokio::time::timeout(Duration::from_millis(500), client.update_config(request)).await {
                            Ok(result) => {
                                match result {
                                    Ok(response) => {
                                        if response.get_ref().success {
                                            println!("Successfully updated config on {}", socket);
                                            break;
                                        } else {
                                            println!("Failed to update config on {}, retrying...", socket);
                                        }
                                    }
                                    Err(_) => {
                                        println!("Failed to update config on {}, retrying...", socket);
                                    }
                                }
                            }
                            Err(_) => {
                                println!("Failed to update config on {}, retrying...", socket);
                            }
                        }
                    }
                }
            }
            Ok(tonic::Response::new(reply))
        } else {
            // redirect to leader
            let leader_socket = get_leader_address().await;
            if leader_socket == None {
                let reply = JoinResponse {
                    success: false,
                };
                return Ok(tonic::Response::new(reply));
            } 
            let mut leader_client = get_channel(leader_socket.unwrap()).await.unwrap();
            match tokio::time::timeout(Duration::from_millis(2000), leader_client.join(request)).await {
                Ok(reply) => {
                    match reply {
                        Ok(response) => {
                            return Ok(response);
                        }
                        Err(_) => {
                            let reply = JoinResponse {
                                success: false,
                            };
                            return Ok(tonic::Response::new(reply));
                        }
                    }
                }
                Err(_) => {
                    let reply = JoinResponse {
                        success: false,
                    };
                    Ok(tonic::Response::new(reply))
                }
            }
        }
    }

    async fn update_config (
        &self,
        request: tonic::Request<UpdateConfigRequest>,
    ) -> Result<tonic::Response<UpdateConfigResponse>, tonic::Status> {
        if !(get_state().await == NodeState::Leader) {
            let addresses = request.get_ref().addresses.clone();
            // convert to sockets
            let mut sockets = Vec::new();
            for address in addresses {
                let socket: SocketAddr = address.parse().unwrap();
                sockets.push(socket);
            }
            // insert to config
            for socket in sockets {
                insert_to_config(socket).await;
            }

            // send ok
            Ok(
                tonic::Response::new(UpdateConfigResponse {
                    success: true,
                })
            )
        } else {
            Ok(
                tonic::Response::new(UpdateConfigResponse {
                    success: false,
                })
            )
        }
    }
    async fn enqueue(
        &self,
        request: tonic::Request<EnqueueRequest>,
    ) -> Result<tonic::Response<EnqueueResponse>, tonic::Status> {
        print_status(format!("Got a enqueue request: {}", request.get_ref().content)).await;
        if get_state().await == NodeState::Leader {
            let content = request.get_ref().content.clone();
            append_to_log(
                LogEntry {
                    term: get_current_term().await,
                    command: "ENQUEUE ".to_string() + &content,
                    index: get_log_len().await,
                }
            ).await;
            set_match_index(SocketAddr::new(HOST, get_port()), get_last_log_index().await).await;
            replicate_log_to_all_followers().await; 
            let reply = EnqueueResponse {
                success: true,
            };
            Ok(tonic::Response::new(reply))
        } else {
            // forward to leader
            let leader_socket = get_leader_address().await;
            if leader_socket == None {
                let reply = EnqueueResponse {
                    success: false,
                };
                return Ok(tonic::Response::new(reply));
            }
            let mut leader_client = get_channel(leader_socket.unwrap()).await.unwrap();
            match tokio::time::timeout(Duration::from_millis(2000), leader_client.enqueue(request)).await {
                Ok(reply) => {
                    match reply {
                        Ok(response) => {
                            return Ok(response);
                        }
                        Err(_) => {
                            let reply = EnqueueResponse {
                                success: false,
                            };
                            return Ok(tonic::Response::new(reply));
                        }
                    }
                }
                Err(_) => {
                    let reply = EnqueueResponse {
                        success: false,
                    };
                    Ok(tonic::Response::new(reply))
                }
            }
        }
    }
    async fn dequeue(
        &self,
        request: tonic::Request<DequeueRequest>,
    ) -> Result<tonic::Response<DequeueResponse>, tonic::Status> {
        print_status("Got a dequeue request").await;
        if get_state().await == NodeState::Leader {
            append_to_log(
                LogEntry {
                    term: get_current_term().await,
                    command: "DEQUEUE".to_string(),
                    index: get_log_len().await,
                }
            ).await;
            set_match_index(SocketAddr::new(HOST, get_port()), get_last_log_index().await).await;
            let res = replicate_log_to_all_followers().await; 
            let reply = DequeueResponse {
                success: true,
                content: res,
            };
            Ok(tonic::Response::new(reply))
        } else {
            // forward to leader
            let leader_socket = get_leader_address().await;
            if leader_socket == None {
                let reply = DequeueResponse {
                    success: false,
                    content: "".to_string(),
                };
                return Ok(tonic::Response::new(reply));
            }
            let mut leader_client = get_channel(leader_socket.unwrap()).await.unwrap();
            match tokio::time::timeout(Duration::from_millis(2000), leader_client.dequeue(request)).await {
                Ok(reply) => {
                    match reply {
                        Ok(response) => {
                            return Ok(response);
                        }
                        Err(_) => {
                            let reply = DequeueResponse {
                                success: false,
                                content: "".to_string(),
                            };
                            return Ok(tonic::Response::new(reply));
                        }
                    }
                }
                Err(_) => {
                    let reply = DequeueResponse {
                        success: false,
                        content: "".to_string(),
                    };
                    Ok(tonic::Response::new(reply))
                }
            }
        }
    }
    async fn read_queue(
        &self,
        request: tonic::Request<ReadQueueRequest>,
    ) -> Result<tonic::Response<ReadQueueResponse>, tonic::Status> {
        print_status("Got a read queue request").await;
        Ok(
            tonic::Response::new(ReadQueueResponse {
                success: true,
                contents: peek_all_queue().await
            })
        ) 
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {    
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() > 1 {
        unsafe {
            PORT = args[1].parse::<u16>().unwrap();
        }
    }
    let addr: SocketAddr = SocketAddr::new(HOST, unsafe { PORT });
    
    // INITIALIZE ALL OPTION VARIABLE
    // NODE INSTANCE
    unsafe {
        NODE_INSTANCE = Some(RwLock::new(Node::new(addr)));
        NODE_INSTANCE.as_ref().unwrap().write().await.load_persistent_from_json();
    }
    // HEARTBEAT TIMER
    unsafe {
        HEARTBEAT_TIMER = Some(RwLock::new(RandomizedTimer::new(LOWER_CEIL_MS, UPPER_CEIL_MS)));
    }

    unsafe {
        STATE_MACHINE = Some(RwLock::new(helper::queue::Queue::new()));
    }

    // CONFIG
    load_config().await;
    
    // OPTION VARIABLE ENDED

    // make join request to random channel in config
    let configlen = get_config().await.node_address_list.len();
    // debug print node address list
    if get_config().await.node_address_list.contains(&addr) {
        print_status("Already in config").await;
    } else if configlen > 0
    && (configlen > 1 || !(get_config().await.node_address_list.contains(&addr)))
    {
        print_status("Sending join request").await;
        let sockets = get_sockets_from_config().await;
        for socket in sockets {
            let mut client = match get_channel(socket).await {
                Some(client) => client,
                None => continue,
            };
            let request = tonic::Request::new(JoinRequest {
                address: addr.to_string(),
            });
            match client.join(request).await {
                Ok(reply) => {
                    if reply.get_ref().success {
                        print_status("Join request success").await;
                        // load config again
                        load_config().await;
                        break;
                    } else {
                        print_status("Join request failed").await;
                        // exit
                        return Ok(());
                    }
                }
                Err(_) => {
                    print_status("Join request failed").await;
                    // exit
                    return Ok(());
                }
            }
        }
    } else {
        // if no node in config, insert self to config
        print_status("No nod, inserting self to config").await;
        insert_to_config(addr).await;
        save_config().await;
    }

    // start server
    let mut mainthreadset = tokio::task::JoinSet::new();
    mainthreadset.spawn(
        async {
            match tonic::transport::Server::builder().add_service(RaftRpcServer::new(RaftRpcImpl::default())).serve(SocketAddr::new(HOST, unsafe { PORT })).await {
                Ok(_) => {},
                Err(_) => {
                    println!("Error starting server, exiting!");
                    // abort thread
                    return;
                }
            }
        }
    );
    print_status("Listening...").await;
    // register_node(addr);
    reset_heartbeat_timer().await;
    mainthreadset.spawn(
        async {
            loop {
                let queuelog = peek_all_queue().await;
                // construct string like [1, 2, 3, 4]
                let mut queuelogstr = String::from("[");
                for i in 0..queuelog.len() {
                    queuelogstr.push_str(&queuelog[i].to_string());
                    if i != queuelog.len() - 1 {
                        queuelogstr.push_str(",");
                    }
                }
                queuelogstr.push_str("]");
                print_status(format!("Queue: {}", queuelogstr)).await;
                tokio::time::sleep(Duration::from_millis(5000)).await;
            }
        }
    );
    mainthreadset.spawn(async {
        loop {
            match get_state().await {
                NodeState::Follower => {
                if is_heartbeat_timer_expired().await {
                        set_current_term(get_current_term().await + 1).await;
                        change_state_to(NodeState::Candidate, "Heartbeat timeout").await;
                        set_voted_for(Some(get_port() as i32)).await;
                        begin_request_voting().await;
                }
                },
                NodeState::Candidate => {

                },
                NodeState::Leader => {
                    // blast heartbeat to all nodes every 100ms
                    replicate_log_to_all_followers().await;
                    tokio::time::sleep(Duration::from_millis(HEARTBEAT_MS)).await;
                }
            }
        }
    });
    while let Some(_) = mainthreadset.join_next().await {}
    print_status("Exiting...").await;
    Ok(())
}