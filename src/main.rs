mod block;
mod blockchain;
mod hash;
mod network;
mod storage;
mod transaction;

use block::Block;
use blockchain::Blockchain;
use network::{NetworkMessage, Node};
use transaction::Transaction;

use std::env;
use std::sync::{Arc, Mutex};
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    println!("🚀 Budlum Node - v0.1.0");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let args: Vec<String> = env::args().collect();
    let port = args
        .iter()
        .position(|r| r == "--port")
        .map(|i| args[i + 1].parse().unwrap())
        .unwrap_or(4001);

    let db_path = args
        .iter()
        .position(|r| r == "--db-path")
        .map(|i| args[i + 1].clone())
        .unwrap_or("budlum_db".to_string());

    let bootstrap_peer = args
        .iter()
        .position(|r| r == "--bootstrap")
        .map(|i| args[i + 1].clone());

    let storage = match storage::Storage::new(&db_path) {
        Ok(s) => Some(s),
        Err(e) => {
            println!("❌ Failed to initialize storage: {}", e);
            None
        }
    };

    let blockchain = Arc::new(Mutex::new(Blockchain::new(2, storage)));

    let mut node = Node::new(blockchain.clone()).unwrap();

    if let Some(addr) = bootstrap_peer {
        if let Err(e) = node.bootstrap(&addr) {
            eprintln!("❌ Failed to bootstrap: {}", e);
        }
    }

    node.listen(port).unwrap();

    if let Some(i) = args.iter().position(|r| r == "--dial") {
        let addr = &args[i + 1];
        node.dial(&addr).expect("Failed to dial");
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
                        "block" => {
                            let block = Block::genesis(); // Demo icin genesis gonderiyoruz
                            client.broadcast("blocks".to_string(), NetworkMessage::Block(block)).await;
                        }
                        "chain" => {
                            let chain = blockchain.lock().unwrap();
                            chain.print_info();
                        }
                        "peers" => {
                            client.list_peers().await;
                        }
                        "sync" => {
                            client.broadcast("blocks".to_string(), NetworkMessage::GetBlocks).await;
                        }
                        "help" => {
                            println!("Commands: tx, block, chain, peers, sync");
                        }
                        _ => {}
                    }
                }
            }
        } => {}
    }
}
