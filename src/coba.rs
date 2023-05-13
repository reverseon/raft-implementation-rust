use std::net::SocketAddr;

pub mod helper;
use helper::rpc::raftrpc::{
    AppendEntriesRequest, raft_rpc_client::RaftRpcClient
};
use tonic::{transport::Channel, Request, Response, Status};

fn convert_socket_addr_to_uri_string(socketaddr: SocketAddr) -> String {
    let mut uri_string = String::from("http://");
    uri_string.push_str(&socketaddr.to_string());
    uri_string
}

#[derive(Debug)]
struct GenericError {
    message: String,
}

impl std::fmt::Display for GenericError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "GenericError: {}", self.message)
    }
}

impl std::error::Error for GenericError {}

fn makeError(message: String) -> Box<dyn std::error::Error> {
    Box::new(GenericError {
        message: message
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>
{
    let socketaddr: SocketAddr = SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        24341 
    );
    let mut client = match RaftRpcClient::connect(
        convert_socket_addr_to_uri_string(socketaddr)
    ).await {
        Ok(client) => client,
        Err(e) => {
            return Err(makeError(String::from("Failed to connect to server")));
        }
    };
    let request = Request::new(
        AppendEntriesRequest {
            term: 1,
            leader_id: 1,
            prev_log_index: 1,
            prev_log_term: 1,
            entries: Vec::new(),
            leader_commit: 1,
        }
    );
    match client.heartbeat(request).await {
        Ok(response) => {
            println!("Response: {:?}", response)
        },
        Err(e) => {
            return Err(makeError(String::from("Failed to send heartbeat")));
        }
    };
    Ok(())
}