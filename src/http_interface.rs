pub mod helper;

use std::net::SocketAddr;

use actix_web::{get, App, HttpResponse, HttpServer, Responder, http::header};
use helper::rpc::raftrpc::{
    LogEntry,
    StatusRequest, StatusResponse,
    raft_rpc_client::RaftRpcClient,
};

use serde::{Deserialize, Serialize};
use helper::node::LogEntryWritable;
use helper::node::Persistent;
use helper::config::Config;
use tonic::transport::Channel;

#[derive(Serialize, Deserialize)]
struct NodeInfo {
    is_online: bool,
    address: String, 
    state: String,
    term: i32,
    leader_address: String,
    log: Vec<LogEntryWritable>,
    commit_index: i32,
    last_applied_index: i32,
    queue: Vec<String>,
}


impl NodeInfo {
    fn new() -> Self {
        NodeInfo {
            is_online: false,
            address: String::new(),
            state: String::new(),
            term: 0,
            leader_address: String::new(),
            log: Vec::new(),
            commit_index: 0,
            last_applied_index: 0,
            queue: Vec::new(),
        }
    }
}

#[get("/")]
async fn status() -> impl Responder {
    let mut cfg = Config::new();
    cfg.load_from_json("cfg/config.json".to_string());
    let sockets = cfg.node_address_list.iter().cloned().collect::<Vec<SocketAddr>>();
    let mut responses : Vec<NodeInfo> = Vec::new();
    for socket in &sockets {
        let mut response = NodeInfo::new();
        match RaftRpcClient::connect(format!("http://{}", socket)).await {
            Ok(mut client_socket) => {
                match client_socket.status(StatusRequest {}).await {
                    Ok(status_response) => {
                        let status_response = status_response.into_inner();
                        response.is_online = true;
                        response.address = format!("{}", socket);
                        response.state = status_response.state;
                        response.term = status_response.term;
                        response.leader_address = format!("{}", status_response.leader_address);
                        response.log = Persistent::log_entry_to_writable(&status_response.log);
                        response.commit_index = status_response.commit_index;
                        response.last_applied_index = status_response.last_applied_index;
                        response.queue = status_response.queue;
                    }
                    Err(_) => {
                        response.is_online = false;
                        response.address = format!("{}", socket);
                    }
                };
            }
            Err(_) => {
                response.is_online = false;
                response.address = format!("{}", socket);
            }
        };
        responses.push(response);
    }
    HttpResponse::Ok()
        .append_header((header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"))
        .append_header((header::ACCESS_CONTROL_ALLOW_METHODS, "GET"))
        .append_header((header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type"))
        .json(responses)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let mut port = 1234;
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() > 1 {
        port = args[1].parse::<u16>().unwrap();
    }
    println!("Starting HTTP Interface on port {}", port);
    HttpServer::new(|| {
        App::new().service(status)
    }).bind(("127.0.0.1", port))?.run().await
}