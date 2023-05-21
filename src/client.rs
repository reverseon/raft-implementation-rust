pub mod helper;

use helper::rpc::raftrpc::{
    EnqueueRequest, EnqueueResponse,
    DequeueRequest, DequeueResponse,
    ReadQueueRequest, ReadQueueResponse,
    raft_rpc_client::RaftRpcClient,
};

use std::io::Write;
use helper::config::Config;
use tonic::{
    transport::Channel,
    Request, Response, Status,
};

use std::net::SocketAddr;

use rand::prelude::SliceRandom;

async fn get_connection(mut sockets: Vec<SocketAddr>) -> Result<RaftRpcClient<Channel>, Status> {
    // randomize sockets
    sockets.shuffle(&mut rand::thread_rng());
    let mut client: Option<RaftRpcClient<Channel>> = None;
    for socket in sockets {
        match RaftRpcClient::connect(format!("http://{}", socket)).await {
            Ok(client_socket) => {
                println!("Connected to {}", socket);
                client = Some(client_socket);
                break;
            }
            Err(_) => continue,
        };
    }
    if client.is_none() {
        return Err(Status::new(tonic::Code::Unavailable, "No node available"));
    }
    Ok(client.unwrap())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("-----------------------------");
    println!("---- RAFT CLUSTER CLIENT ----");
    println!("-----------------------------");
    let mut clusterconfig = Config::new();
    clusterconfig.load_from_json("cfg/config.json".to_string());
    let mut client: Option<RaftRpcClient<Channel>> = None;
    // convert hashset to vector
    let mut sockets = clusterconfig.node_address_list.iter().cloned().collect::<Vec<SocketAddr>>();
    // print whole cluster members
    println!("Cluster Members");
    println!("----------------------------");
    for socket in &sockets {
        println!("{}", socket);
    }

    println!("----------------------------");
    println!("Connecting...");
    client = match get_connection(sockets.clone()).await {
        Ok(client) => Some(client),
        Err(_) => None,
    }; 
    if client.is_none() {
        println!("Client failed to connect to any node");
        return Ok(());
    }
    loop {
        if client.is_none() {
            println!("----------------------------");
            println!("Reconnecting...");
            client = match get_connection(sockets.clone()).await {
                Ok(client) => Some(client),
                Err(_) => None,
            }; 
            if client.is_none() {
                println!("Client failed to connect to any node");
                return Ok(());
            }
        }
        println!("----------------------------");
        println!("Available Commands");
        println!("----------------------------");
        println!("1. Enqueue");
        println!("2. Dequeue");
        println!("3. ReadQueue");
        println!("----------------------------");
        print!("Enter command number: ");
        // flush stdout
        std::io::stdout().flush().unwrap();
        let mut command = String::new();
        tokio::io::AsyncBufReadExt::read_line(&mut tokio::io::BufReader::new(tokio::io::stdin()), &mut command).await.unwrap();
        let command = match command.trim().parse::<i32>() {
            Ok(num) => num,
            Err(_) => {
                println!("Invalid command");
                continue;
            }
        };
        match command {
            1 => {
                println!("----------------------------");
                println!("|         Enqueue          |"); 
                println!("----------------------------");
                print!("Enqueue String: ");
                std::io::stdout().flush().unwrap();
                let mut enqueue_string = String::new();
                tokio::io::AsyncBufReadExt::read_line(&mut tokio::io::BufReader::new(tokio::io::stdin()), &mut enqueue_string).await.unwrap();
                // strip newline from enqueue_string
                enqueue_string = enqueue_string.trim().to_string();
                println!("----------------------------");
                println!("Input: {}", enqueue_string);
                println!("----------------------------");
                println!("Sending request...");
                let request = Request::new(EnqueueRequest {
                    content: enqueue_string,
                });
                match client.as_mut().unwrap().enqueue(request).await {
                    Ok(response) => {
                        let response: Response<EnqueueResponse> = response;
                        if response.get_ref().success {
                            println!("Enqueue successful!");
                        } else {
                            println!("Enqueue failed!");
                        }
                    }
                    Err(_) => {
                        // retry sending request again
                        println!("Error sending request! connecting to another node...");
                        client = match get_connection(sockets.clone()).await {
                            Ok(client) => Some(client),
                            Err(_) => None,
                        };
                    }
                };
            }
            2 => {
                println!("----------------------------");
                println!("|         Dequeue          |"); 
                println!("----------------------------");
                println!("Sending request...");
                let request = Request::new(DequeueRequest {});
                match client.as_mut().unwrap().dequeue(request).await {
                    Ok(response) => {
                        let response: Response<DequeueResponse> = response;
                        if response.get_ref().success {
                            println!("Dequeue successful!");
                            println!("----------------------------");
                            println!("Dequeued String: {}", response.get_ref().content);
                            println!("----------------------------");
                        } else {
                            println!("Dequeue failed!");
                        }
                    }
                    Err(_) => {
                        println!("Error sending request! connecting to another node...");
                        client = match get_connection(sockets.clone()).await {
                            Ok(client) => Some(client),
                            Err(_) => None,
                        };
                    }
                };
            }
            3 => {
                println!("----------------------------");
                println!("|        ReadQueue         |");
                println!("----------------------------");
                println!("Sending request...");
                let request = Request::new(ReadQueueRequest {});
                match client.as_mut().unwrap().read_queue(request).await {
                    Ok(response) => {
                        let response: Response<ReadQueueResponse> = response;
                        if response.get_ref().success {
                            println!("ReadQueue successful!");
                            println!("----------------------------");
                            let queue = &response.get_ref().contents;
                            // print it like [a, b, c]
                            print!("Queue: [");
                            for (i, content) in queue.iter().enumerate() {
                                if i == queue.len() - 1 {
                                    print!("{}", content);
                                } else {
                                    print!("{}, ", content);
                                }
                            }
                            println!("]");
                            println!("----------------------------");
                        } else {
                            println!("ReadQueue failed!");
                        }
                    }
                    Err(_) => {
                        println!("Error sending request! connecting to another node...");
                        client = match get_connection(sockets.clone()).await {
                            Ok(client) => Some(client),
                            Err(_) => None,
                        };
                    }
                };

            }
            _ => {
                println!("Invalid command");
            }
        }
    }
}
