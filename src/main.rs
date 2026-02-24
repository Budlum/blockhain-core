mod account;
mod block;
mod blockchain;
mod cli;
mod consensus;
mod crypto;
mod encoding;
mod genesis;
mod hash;
mod mempool;
mod network;
mod snapshot;
mod storage;
mod transaction;

#[cfg(test)]
mod integration_tests;
use block::{Block, BlockHeader};
use blockchain::Blockchain;
use clap::Parser;
use cli::{ConsensusType, NodeConfig};
use consensus::{ConsensusEngine, PoAEngine, PoSEngine, PoWEngine};
use network::{NetworkMessage, Node};
use std::sync::{Arc, Mutex};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use transaction::Transaction;
#[tokio::main]
async fn main() {
    let config = NodeConfig::parse();
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    println!("Budlum Node - v0.2.0 (Framework Edition)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Configuration:");
    println!("   Port: {}", config.port);
    println!("   Consensus: {:?}", config.consensus);
    println!("   Privacy: {:?}", config.privacy);
    println!("   DB Path: {}", config.db_path);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    let consensus: Arc<dyn ConsensusEngine> = match config.consensus {
        ConsensusType::PoW => {
            println!(" PoW mode - difficulty: {}", config.difficulty);
            Arc::new(PoWEngine::new(config.difficulty))
        }
        ConsensusType::PoS => {
            println!("🥩 PoS mode - min stake: {}", config.min_stake);
            let pos_config = crate::consensus::pos::PoSConfig {
                 min_stake: config.min_stake,
                 ..Default::default()
            };
            Arc::new(PoSEngine::new(pos_config, None))
        }
        ConsensusType::PoA => {
            println!("👥 PoA mode");
            Arc::new(PoAEngine::new(crate::consensus::poa::PoAConfig::default(), None))
        }
    };
    let storage = match storage::Storage::new(&config.db_path) {
        Ok(s) => Some(s),
        Err(e) => {
            println!("Failed to initialize storage: {}", e);
            None
        }
    };

    let pruning_manager = snapshot::PruningManager::new(1000, 100, "./data/snapshots".to_string());

    let blockchain = Arc::new(Mutex::new(Blockchain::new(
        consensus,
        storage,
        config.chain_id,
        Some(pruning_manager),
    )));
    
    if let ConsensusType::PoA = config.consensus {
         let validators = config.load_validators();
         if !validators.is_empty() {
             println!("👥 Initializing PoA validators: {:?}", validators);
             let mut bc = blockchain.lock().unwrap();
             for addr in validators {
                 let mut v = crate::account::Validator::new(addr.clone(), 0);
                 v.active = true;
                 bc.state.validators.insert(addr, v);
             }
         } else {
             println!(" No validators configured!");
         }
    }

    let mut node = Node::new(blockchain.clone()).unwrap();
    if let Some(ref addr) = config.bootstrap {
        if let Err(e) = node.bootstrap(addr) {
            eprintln!("Failed to bootstrap: {}", e);
        }
    }
    node.listen(config.port).unwrap();
    if let Some(ref addr) = config.dial {
        node.dial(addr).expect("Failed to dial");
    }
    let client = node.get_client();
    let peer_id = node.peer_id;
    tokio::select! {
        _ = node.run() => {},
        _ = async {
            let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
            let mut line = String::new();
            client.subscribe("blocks".to_string()).await;
            client.subscribe("transactions".to_string()).await;
            loop {
                line.clear();
                use tokio::io::AsyncBufReadExt;
                if stdin.read_line(&mut line).await.is_ok() {
                    let cmd = line.trim();
                    match cmd {
                        "tx" => {
                            let tx = Transaction::new(
                                peer_id.to_string(),
                                "recipient".to_string(),
                                10,
                                b"demo tx".to_vec(),
                            );
                            client.broadcast("transactions".to_string(), NetworkMessage::Transaction(tx)).await;
                        }
                        "block" | "mine" => {
                            let mut chain = blockchain.lock().unwrap();
                            chain.produce_block(peer_id.to_string());
                        }
                        "chain" => {
                            let chain = blockchain.lock().unwrap();
                            chain.print_info();
                        }
                        "peers" => {
                            client.list_peers().await;
                        }
                        "sync" => {
                            let msg = NetworkMessage::GetHeaders {
                                locator: Vec::new(),
                                limit: 2000,
                            };
                            client.broadcast("blocks".to_string(), msg).await;
                        }
                        "help" => {
                            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                            println!("📖 Commands:");
                            println!("   tx    - Send demo transaction");
                            println!("   mine  - Produce new block");
                            println!("   chain - Show blockchain info");
                            println!("   peers - List connected peers");
                            println!("   sync  - Request chain sync");
                            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        }
                        _ => {}
                    }


                }
            }
        } => {}
    }
}
