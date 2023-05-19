pub mod helper;

use std::net::{SocketAddr, Ipv4Addr, IpAddr};
use std::sync::{Arc};
use tokio::sync::{RwLock};
use std::time::{Instant, Duration};
use std::collections::{HashSet, HashMap};

use helper::config::Config;
use helper::node::Node;
use helper::timer::{
    RandomizedTimer,
    get_randomized_duration,
};
use helper::state::NodeState;
use helper::election::ElectionState;
use helper::rpc::raftrpc::raft_rpc_server::{RaftRpc, RaftRpcServer};
use helper::rpc::raftrpc::{LogEntry, AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse, Empty, JoinRequest, JoinResponse, UpdateConfigRequest, UpdateConfigResponse};
use helper::rpc::raftrpc::raft_rpc_client::RaftRpcClient;
use tonic::{transport::{Server, Channel, Endpoint}, Request, Response, Status};

const CONFIG_PATH: &str = "cfg/config.json";
static mut CONFIG : Option<RwLock<Config>> = None;
const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
static mut PORT: u16 = 24341;
static mut NODE_INSTANCE: Option<RwLock<Node>> = None;
static mut HEARTBEAT_TIMER: Option<RwLock<RandomizedTimer>> = None;
static mut ELECTION_TIMER: Option<RwLock<RandomizedTimer>> = None;
const LOWER_CEIL_MS: u64 = 150;
const UPPER_CEIL_MS: u64 = 300;
const HEARTBEAT_MS: u64 = 100;


// HELPER FUNCTION THAT NEEDS DIRECT REFERENCE

async fn print_status<S: AsRef<str>>(msg: S) {
    println!("[{:?} | {}:{} | {}]: {:?}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(), HOST, unsafe{PORT}, unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.state.get_state()
    }, msg.as_ref());
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
async fn get_current_term () -> i32 {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.current_term
    }
}

async fn set_current_term (term: i32) {
    unsafe {
        NODE_INSTANCE.as_mut().unwrap().write().await.current_term = term;
    }
}

async fn get_voted_for () -> Option<i32> {
    unsafe {
        NODE_INSTANCE.as_ref().unwrap().read().await.voted_for
    }
}

async fn set_voted_for (voted_for: Option<i32>) {
    unsafe {
        NODE_INSTANCE.as_mut().unwrap().write().await.voted_for = voted_for;
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

async fn reset_election_timer () {
    unsafe {
    let mut timer = ELECTION_TIMER.as_ref().unwrap().write().await;
    timer.reset();
    }
}

async fn reset_heartbeat_timer () {
    unsafe {
    let mut timer = HEARTBEAT_TIMER.as_mut().unwrap().write().await;
    timer.reset();
    }
}

async fn is_election_timer_expired () -> bool {
    unsafe {
    let timer = ELECTION_TIMER.as_mut().unwrap().read().await;
    timer.is_expired()
    }
}

async fn is_heartbeat_timer_expired () -> bool {
    unsafe {
    let timer = HEARTBEAT_TIMER.as_ref().unwrap().read().await;
    timer.is_expired()
    }
}


async fn get_sockets_from_config () -> Vec<SocketAddr> {
    unsafe {
        CONFIG.as_ref().unwrap().read().await.node_address_list.iter().cloned().collect::<Vec<SocketAddr>>()
    }
}

async fn get_channel(socket: SocketAddr) -> Option<RaftRpcClient<Channel>> {
    match tokio::time::timeout(Duration::from_millis(HEARTBEAT_MS), 
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


async fn begin_request_voting() {
    // if config is <= 1, immediately become leader
    if get_config().await.node_address_list.len() <= 1 {
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
        for socket in sockets {
            if socket == SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), get_port()) {
                continue;
            }
            let voted_ids_clone = Arc::clone(&voted_ids);
            tokio::spawn(async move {
                let mut client = match  get_channel(socket).await {
                    Some(client) => {
                        println!("Connected to {}", socket);
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
                match client.request_vote(request).await {
                    Ok(response) => {
                        // if response.get_ref().vote_granted {
                            // voted_ids_clone.lock().unwrap().insert(new_addr_clone[i].port() as i32);
                            // print_status(format!("Received vote from {}", new_addr_clone[i]));
                        // }
                        if get_state().await == NodeState::Candidate && response.get_ref().term == get_current_term().await && response.get_ref().vote_granted {
                            let mut vidcw = voted_ids_clone.write().await;
                            vidcw.insert(socket.port() as i32);
                            drop(vidcw);
                            if voted_ids_clone.read().await.len() > majoritytally {
                                change_state_to(NodeState::Leader, format!("Won election")).await;
                                blast_heartbeat().await;
                            } 
                            print_status(format!("Received vote from {}", socket)).await;
                        } else if response.get_ref().term > get_current_term().await {
                            change_state_to(NodeState::Follower, format!("Received higher term from {}", socket)).await;
                            set_leader_address(Some(socket)).await;
                            reset_heartbeat_timer().await;
                            set_current_term(response.get_ref().term).await;
                            set_voted_for(None).await;
                        }
                    }
                    Err(_) => {
                        print_status(format!("Error sending request to {}", socket)).await;
                    }
                };
            });
        }
        // timer
        reset_election_timer().await;
        loop {
            if get_state().await != NodeState::Candidate {
                break;
            }
            if is_election_timer_expired().await {
                break;
            }
        }
        print_status("No winner, resetting election").await;        
        print_status(
            format!("Election ended with vote: {:?} at term: {}", voted_ids.read().await.iter().map(|x| x.to_string()).collect::<Vec<String>>(), get_current_term().await)
        ).await;
        set_current_term(get_current_term().await + 1).await;
        set_voted_for(Some(get_port() as i32)).await;

    }
    
} 

async fn blast_heartbeat() {
    if get_state().await != NodeState::Leader {
        return;
    }
    // send heartbeat to all nodes in different threads
    print_status("Sending heartbeat").await;
    // let addresses = get_config().node_address_list;
    // let mut clients = Vec::new();
    // let mut new_addresses = Vec::new();
    // for address in addresses.clone() {
    //     if address != SocketAddr::new(HOST, get_port()) {
    //         let cts = match RaftRpcClient::connect(format!("http://{}", address)).await {
    //             Ok(cts) => {
    //                 clients.push(cts);
    //                 new_addresses.push(address);
    //             },
    //             Err(_) => {
    //                 print_status(format!("Error connecting to {}", address));
    //                 continue;
    //             }
    //         };
    //     }
    // }
    // update_client_pool().await;
    let socketsclone = get_sockets_from_config().await.iter().cloned().collect::<Vec<SocketAddr>>();
    for socket in socketsclone { 
        if socket == SocketAddr::new(HOST, get_port()) {
            continue;
        }
        // print_status(format!("Sending heartbeat to {}", socket)).await;
        tokio::spawn( async move {
            let request = Request::new(
                AppendEntriesRequest {
                    term: get_current_term().await,
                    leader_id: get_port() as i32,
                    prev_log_index: get_last_log_index().await,
                    prev_log_term: get_current_term().await,
                    entries: Vec::new(),
                    leader_commit: 0,
                    leader_address: format!("{}", SocketAddr::new(HOST, get_port())),
                }
            ); 
            let mut channel = match get_channel(socket as SocketAddr).await {
                Some(channel) => channel,
                None => {
                    print_status(format!("Error connecting to {}", socket)).await;
                    return;
                }
            };
            match channel.heartbeat(request).await {
                Ok(_) => {
                }
                Err(_) => {
                }
            };
        });
    }

}

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
        // println!("Request term: {} with ID: {}", request.get_ref().term, request.get_ref().candidate_id);
        // println!("Current term: {}", get_current_term());
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
            print_status(format!( "Got a heartbeat from {}",request.get_ref().leader_id )).await;
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
                    // node addresses list from hashset to vector<string>
                    let mut addresses: Vec<String> = Vec::new();
                    for address in get_config().await.node_address_list.iter() {
                        addresses.push(address.to_string());
                    }
                
                    let request = tonic::Request::new(UpdateConfigRequest {
                        addresses
                    });

                    match client.update_config(request).await {
                        Ok(_) => {
                        }
                        Err(_) => {
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
            match leader_client.join(request).await {
                Ok(reply) => {
                    Ok(reply)
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
    }
    // HEARTBEAT TIMER
    unsafe {
        HEARTBEAT_TIMER = Some(RwLock::new(RandomizedTimer::new(LOWER_CEIL_MS, UPPER_CEIL_MS)));
    }
    // ELECTION TIMER
    unsafe {
        ELECTION_TIMER = Some(RwLock::new(RandomizedTimer::new(LOWER_CEIL_MS, UPPER_CEIL_MS)));
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
        print_status("No node or only self in config, inserting self to config").await;
        insert_to_config(addr).await;
        save_config().await;
    }

    // start server

    let srv_thread = tokio::task::spawn(
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
    loop {
        match get_state().await {
            NodeState::Follower => {
               if is_heartbeat_timer_expired().await {
                    unsafe {
                        // for debug
                        print_status(format!("Time: {} ms", HEARTBEAT_TIMER.as_ref().unwrap().read().await.get_duration().as_millis())).await;
                    }
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
                blast_heartbeat().await;
                tokio::time::sleep(Duration::from_millis(HEARTBEAT_MS)).await;
            }
        }
        if srv_thread.is_finished() {
            print_status("Server Stopped, exiting!").await;
            break;
        }
    }
    Ok(())
}