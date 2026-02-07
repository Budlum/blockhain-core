
use crate::network::protocol::NetworkMessage;
use libp2p::{
    futures::StreamExt,
    gossipsub, identify, identity,
    kad::{
        store::MemoryStore, Behaviour as Kademlia, Config as KademliaConfig, Event as KademliaEvent,
    },
    mdns, noise, ping,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm,
};
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tracing::{info, warn};

#[derive(NetworkBehaviour)]
pub struct BudlumBehaviour {
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    mdns: mdns::tokio::Behaviour,
    gossipsub: gossipsub::Behaviour,
    kad: Kademlia<MemoryStore>,
}

use tokio::sync::mpsc;

use crate::Block;
use crate::Blockchain;
use std::sync::{Arc, Mutex};

pub enum NodeCommand {
    Subscribe(String),
    Broadcast(String, NetworkMessage),
    ListPeers,
}

#[derive(Clone)]
pub struct NodeClient {
    sender: mpsc::Sender<NodeCommand>,
    pub peer_id: PeerId,
}

impl NodeClient {
    pub async fn subscribe(&self, topic: String) {
        let _ = self.sender.send(NodeCommand::Subscribe(topic)).await;
    }

    pub async fn broadcast(&self, topic: String, msg: NetworkMessage) {
        let _ = self.sender.send(NodeCommand::Broadcast(topic, msg)).await;
    }

    pub async fn list_peers(&self) {
        let _ = self.sender.send(NodeCommand::ListPeers).await;
    }
}

#[tokio::test]
async fn test_node_creation() {
    let blockchain = Arc::new(Mutex::new(Blockchain::new(2, None)));
    let node = Node::new(blockchain);
    assert!(node.is_ok());
}

pub struct Node {
    swarm: Swarm<BudlumBehaviour>,
    command_rx: mpsc::Receiver<NodeCommand>,
    command_tx: mpsc::Sender<NodeCommand>,
    pub peer_id: PeerId,
    pub blockchain: Arc<Mutex<Blockchain>>,
}

impl Node {
    pub fn new(blockchain: Arc<Mutex<Blockchain>>) -> Result<Self, Box<dyn Error>> {
        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        info!("🔑 Node ID: {}", peer_id);

        let message_id_fn = |message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        };

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            .build()
            .map_err(|msg| std::io::Error::new(std::io::ErrorKind::Other, msg))?;

        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        )?;

        let swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id(),
                )?;

                let kad_store = MemoryStore::new(key.public().to_peer_id());
                let kad_config = KademliaConfig::default();
                let kademlia =
                    Kademlia::with_config(key.public().to_peer_id(), kad_store, kad_config);

                let identify = identify::Behaviour::new(identify::Config::new(
                    "/budlum/1.0.0".to_string(),
                    key.public(),
                ));

                Ok(BudlumBehaviour {
                    ping: ping::Behaviour::new(
                        ping::Config::new().with_interval(Duration::from_secs(15)),
                    ),
                    identify,
                    mdns,
                    gossipsub,
                    kad: kademlia,
                })
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        let (command_tx, command_rx) = mpsc::channel(32);

        Ok(Node {
            swarm,
            peer_id,
            command_tx,
            command_rx,
            blockchain,
        })
    }

    pub fn get_client(&self) -> NodeClient {
        NodeClient {
            sender: self.command_tx.clone(),
            peer_id: self.peer_id,
        }
    }

    pub fn listen(&mut self, port: u16) -> Result<(), Box<dyn Error>> {
        let addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", port).parse()?;
        self.swarm.listen_on(addr)?;
        info!("👂 Listening on port {}", port);
        Ok(())
    }

    pub fn dial(&mut self, addr: &str) -> Result<(), Box<dyn Error>> {
        let remote: Multiaddr = addr.parse()?;
        self.swarm.dial(remote)?;
        info!("📞 Dialing {}", addr);
        Ok(())
    }

    pub fn bootstrap(&mut self, addr: &str) -> Result<(), Box<dyn Error>> {
        let multiaddr: Multiaddr = addr.parse()?;

        let peer_id = match multiaddr
            .iter()
            .find(|p| matches!(p, libp2p::multiaddr::Protocol::P2p(_)))
        {
            Some(libp2p::multiaddr::Protocol::P2p(peer_id)) => peer_id,
            _ => return Err("Bootstrap address must contain /p2p/<ID>".into()),
        };

        info!("👢 Bootstrapping via {}", addr);
        self.swarm
            .behaviour_mut()
            .kad
            .add_address(&peer_id, multiaddr);
        self.swarm.behaviour_mut().kad.bootstrap()?;
        Ok(())
    }

    pub async fn run(&mut self) {
        info!("🚀 Node running...");

        loop {
            tokio::select! {
                cmd = self.command_rx.recv() => {
                    if let Some(cmd) = cmd {
                        match cmd {
                            NodeCommand::Subscribe(topic) => {
                                let topic = gossipsub::IdentTopic::new(topic);
                                if let Err(e) = self.swarm.behaviour_mut().gossipsub.subscribe(&topic) {
                                    warn!("Failed to subscribe: {}", e);
                                } else {
                                    info!("✅ Subscribed to topic: {}", topic);
                                }
                            }
                            NodeCommand::Broadcast(topic, msg) => {
                                let topic = gossipsub::IdentTopic::new(topic);
                                let data = msg.to_bytes();
                                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic.clone(), data) {
                                    warn!("Failed to publish: {}", e);
                                } else {
                                    info!("📢 Broadcasted to {}: {:?}", topic, msg);
                                }
                            }
                            NodeCommand::ListPeers => {
                                let peers: Vec<_> = self.swarm.behaviour().gossipsub.all_peers().collect();
                                info!("👥 Connected peers: {:?}", peers.len());
                                for (peer, _topics) in peers {
                                    info!(" - {}", peer);
                                }
                            }
                        }
                    }
                }
                event = self.swarm.select_next_some() => {
                    match event {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!("📍 Listening on {}", address);
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            info!("🤝 Connected to {}", peer_id);
                            let chain = self.blockchain.lock().unwrap();
                            info!("DEBUG: Connected to {}, Chain length: {}", peer_id, chain.chain.len());
                            if chain.chain.len() == 1 {
                                drop(chain); // Mutex kilidini bırak
                                info!("🔌 New connection, requesting blocks...");
                                let topic = gossipsub::IdentTopic::new("blocks");
                                let msg = NetworkMessage::GetBlocks;
                                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, msg.to_bytes()) {
                                    warn!("Failed to request blocks: {}", e);
                                }
                            }
                        }
                        SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            warn!("👋 Disconnected from {}", peer_id);
                        }
                        SwarmEvent::Behaviour(BudlumBehaviourEvent::Ping(event)) => {
                        }
                        SwarmEvent::Behaviour(BudlumBehaviourEvent::Mdns(event)) => {
                            match event {
                                mdns::Event::Discovered(peers) => {
                                    for (peer_id, addr) in peers {
                                        info!("🔍 mDNS discovered: {} at {}", peer_id, addr);
                                        self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                                        if let Err(e) = self.swarm.dial(addr.clone()) {
                                            warn!("Failed to dial discovered peer: {}", e);
                                        }
                                    }
                                }
                                mdns::Event::Expired(peers) => {
                                    for (peer_id, _) in peers {
                                        info!("⏰ mDNS expired: {}", peer_id);
                                        self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                                    }
                                }
                            }
                        }
                        SwarmEvent::Behaviour(BudlumBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                            propagation_source: peer_id,
                            message_id: id,
                            message,
                        })) => {
                            info!("📨 Received from {}: id={}", peer_id, id);
                            if let Ok(msg) = NetworkMessage::from_bytes(&message.data) {
                                match msg {
                                    NetworkMessage::Block(block) => {
                                        info!("📦 BLOCK: #{} Hash: {}...", block.index, &block.hash[..8]);
                                        let mut chain = self.blockchain.lock().unwrap();
                                        if block.index == chain.chain.len() as u64 {
                                            info!("✅ Added block #{} to local chain", block.index);
                                        }
                                    }
                                    NetworkMessage::Transaction(tx) => {
                                        info!("💸 TX: {}->{} Amount: {}",
                                            &tx.from[..8], &tx.to[..8], tx.amount);
                                        let mut chain = self.blockchain.lock().unwrap();
                                        chain.add_transaction(tx);
                                    }
                                    NetworkMessage::GetBlocks => {
                                        info!("📥 Received GetBlocks request from {}", peer_id);
                                        let chain = self.blockchain.lock().unwrap();
                                        let blocks = chain.chain.clone();

                                        let response = NetworkMessage::Chain(blocks);
                                        let topic = gossipsub::IdentTopic::new("blocks");
                                        let data = response.to_bytes();
                                        if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, data) {
                                            warn!("Failed to send chain: {}", e);
                                        }
                                    }
                                    NetworkMessage::Chain(blocks) => {
                                        info!("⛓️ Received Chain with {} blocks form {}", blocks.len(), peer_id);
                                        let mut chain = self.blockchain.lock().unwrap();
                                        if blocks.len() > chain.chain.len() {
                                            if chain.is_valid_chain(&blocks) {
                                                info!("✅ Replaced local chain with longer chain (len: {})", blocks.len());
                                                chain.chain = blocks;
                                            } else {
                                                warn!("❌ Received invalid chain!");
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        SwarmEvent::Behaviour(BudlumBehaviourEvent::Identify(event)) => {
                            if let identify::Event::Received { info, .. } = event {
                                info!("🆔 Received identity from {:?}", info.public_key.to_peer_id());
                                for addr in info.listen_addrs {
                                    self.swarm.behaviour_mut().kad.add_address(&info.public_key.to_peer_id(), addr);
                                }
                            }
                        }
                        SwarmEvent::Behaviour(BudlumBehaviourEvent::Kad(event)) => {
                            match event {
                                KademliaEvent::RoutingUpdated { peer, .. } => {
                                    info!("🌐 Kademlia: Routing updated for peer {}", peer);
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
