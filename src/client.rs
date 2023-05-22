pub mod helper;

use helper::rpc::raftrpc::{
    EnqueueRequest, EnqueueResponse,
    DequeueRequest, DequeueResponse,
    ReadQueueRequest, ReadQueueResponse,
    raft_rpc_client::RaftRpcClient,
};

use std::{io::Write};
use helper::config::Config;
use tonic::{
    transport::Channel,
    Request, Response, Status,
};

use std::net::SocketAddr;

use rand::prelude::SliceRandom;

async fn get_single_connection(socket: SocketAddr) -> Result<RaftRpcClient<Channel>, Status> {
    match RaftRpcClient::connect(format!("http://{}", socket)).await {
        Ok(client_socket) => {
            println!("Connected to {}", socket);
            Ok(client_socket)
        }
        Err(_) => Err(Status::new(tonic::Code::Unavailable, "No node available")),
    }
}

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
    let sockets = clusterconfig.node_address_list.iter().cloned().collect::<Vec<SocketAddr>>();
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
        println!("4. Reconnect to random node");
        println!("5. Reconnect to specific node");
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
                    content: enqueue_string.clone(),
                    is_retry: false,
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
                        println!("Retry sending request 3 times...");
                        for _ in 0..3 {
                            let request = Request::new(EnqueueRequest {
                                content: enqueue_string.clone(),
                                is_retry: true,
                            });
                            match client.as_mut().unwrap().enqueue(request).await {
                                Ok(response) => {
                                    let response: Response<EnqueueResponse> = response;
                                    if response.get_ref().success {
                                        println!("Enqueue successful!");
                                        break;
                                    } else {
                                        println!("Enqueue failed!");
                                    }
                                }
                                Err(_) => {
                                    println!("Error sending request! retrying...");
                                }
                            };
                        }
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
                let request = Request::new(DequeueRequest {
                    is_retry: false,
                });
                match client.as_mut().unwrap().dequeue(request).await {
                    Ok(response) => {
                        let response: Response<DequeueResponse> = response;
                        if response.get_ref().success {
                            println!("Dequeue successful!");
                            println!("----------------------------");
                            println!("Dequeued String: {}", response.get_ref().content);
                        } else {
                            println!("Dequeue failed!");
                        }
                    }
                    Err(_) => {
                        println!("Error sending request! retrying...");
                        // retry sending request again
                        for _ in 0..3 {
                            let request = Request::new(DequeueRequest {
                                is_retry: true,
                            });
                            match client.as_mut().unwrap().dequeue(request).await {
                                Ok(response) => {
                                    let response: Response<DequeueResponse> = response;
                                    if response.get_ref().success {
                                        println!("Dequeue successful!");
                                        println!("----------------------------");
                                        println!("Dequeued String: {}", response.get_ref().content);
                                        println!("----------------------------");
                                        break;
                                    } else {
                                        println!("Dequeue failed!");
                                    }
                                }
                                Err(_) => {
                                    println!("Error sending request! retrying...");
                                }
                            };
                        }
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
                        println!("Error sending request! retrying...");
                        // retry sending request again
                        for _ in 0..3 {
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
                                        break;
                                    } else {
                                        println!("ReadQueue failed!");
                                    }
                                }
                                Err(_) => {
                                    println!("Error sending request! retrying...");
                                }
                            };
                        }
                        println!("Error sending request! connecting to another node...");
                        client = match get_connection(sockets.clone()).await {
                            Ok(client) => Some(client),
                            Err(_) => None,
                        };
                    }
                };

            }
            4 => {
                println!("Reconnecting...");
                client = get_connection(sockets.clone()).await.ok();
            }
            5 => {
                println!("----------------------------");
                println!("|        Node List         |");
                println!("----------------------------");
                for i in 0..sockets.len() {
                    println!("Node {}: {}", i, sockets[i]);
                }
                println!("----------------------------");
                print!("Enter node number to connect to: ");
                // flush stdout
                std::io::stdout().flush().unwrap();
                let mut node_number = String::new(); 
                std::io::stdin().read_line(&mut node_number).unwrap();
                let node_number: usize = match node_number.trim().parse() {
                    Ok(num) => num,
                    Err(_) => {
                        println!("Invalid node number");
                        continue;
                    }
                };
                if node_number >= sockets.len() {
                    println!("Invalid node number");
                    continue;
                }
                println!("Connecting to node {}...", node_number);
                for _ in 0..3 {
                    match get_single_connection(sockets[node_number].clone()).await {
                        Ok(client_in) => {
                            client = Some(client_in); break;
                        }
                        Err(_) => {
                            client = None;
                            println!("Error connecting to node {}! retrying...", node_number);
                        }
                    };
                }
                if client.is_none() {
                    println!("Error connecting to node {}! connecting to another node...", node_number);
                    client = match get_connection(sockets.clone()).await {
                        Ok(client) => Some(client),
                        Err(_) => None,
                    };
                }
            }
            _ => {
                println!("Invalid command");
            }
        }
    }
}
