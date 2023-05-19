use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::net::SocketAddr;
use tonic::transport::{Channel, Endpoint};
use super::rpc::raftrpc::raft_rpc_client::RaftRpcClient;

#[derive(Clone)]
pub struct LazyReconnectingChannelPool {
    pub sockets: Arc<Mutex<HashSet<SocketAddr>>>,
    pub channels: Arc<Mutex<HashMap<SocketAddr, Option<RaftRpcClient<Channel>>>>>,
}

impl LazyReconnectingChannelPool {
    pub fn new() -> Self {
        LazyReconnectingChannelPool {
            sockets: Arc::new(Mutex::new(HashSet::new())),
            channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_channel(&self, socket: SocketAddr) -> Option<RaftRpcClient<Channel>> {
        let mut sockets = self.sockets.lock().await;
        let mut channels = self.channels.lock().await;
        sockets.insert(socket);
        if channels.get(&socket).unwrap().is_none() {
            println!("Masuk sini bos");
            let channel = match RaftRpcClient::connect(
                format!("http://{}", socket.to_string())
            ).await {
                Ok(channel) => Some(channel),
                Err(_) => None,
            };
            channels.insert(socket, channel.clone());
            // debug print channels
        }
        channels.get(&socket).unwrap().clone()
    }

    pub async fn add_channel(&self, socket: SocketAddr) {
        let mut sockets = self.sockets.lock().await;
        let mut channels = self.channels.lock().await;
        sockets.insert(socket);
        if !channels.contains_key(&socket) {
            channels.insert(socket, None);
        }
    }

    pub async fn remove_channel(&self, socket: SocketAddr) {
        let mut sockets = self.sockets.lock().await;
        let mut channels = self.channels.lock().await;
        sockets.remove(&socket);
        channels.remove(&socket);
    }

    pub async fn get_random_channel(&self) -> Option<RaftRpcClient<Channel>> {
        let sockets = self.sockets.lock().await;
        let channels = self.channels.lock().await;
        let socket = sockets.iter().next().unwrap();
        match channels.get(socket).unwrap().clone() {
            Some(channel) => Some(channel),
            None => {
                let channel = match RaftRpcClient::connect(
                    format!("http://{}", socket.to_string())
                ).await {
                    Ok(channel) => Some(channel),
                    Err(_) => None,
                };
                channel
            }
        }
    }
}


