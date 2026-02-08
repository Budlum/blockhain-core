# Budlum Blockchain Core

**Budlum Core** is a production-grade, modular blockchain framework written in Rust. It serves as a high-performance Layer-1 blockchain featuring pluggable consensus engines (PoW, PoS, PoA), a hardened libp2p-based networking stack, and an atomic, account-based state model.

The architecture emphasizes **security**, **modularity**, and **readability**, making it an ideal foundation for custom blockchain networks or educational study of advanced distributed ledger technology.

---

## 📚 Table of Contents

- [Architecture Overview](#architecture-overview)
- [Quick Start](#quick-start)
- [Core Components Deep Dive](#core-components-deep-dive)
    - [1. Data Structures](#1-data-structures)
    - [2. Consensus Engines](#2-consensus-engines)
    - [3. Networking Layer](#3-networking-layer)
    - [4. State Management](#4-state-management)
    - [5. Cryptography & Security](#5-cryptography--security)
- [CLI Reference](#cli-reference)
- [Development Guide](#development-guide)

---

## 🏗️ Architecture Overview

Budlum Core follows a layered architecture where modules are loosely coupled through Rust `traits`.

```mermaid
graph TD
    User((User)) --> CLI[CLI / API Layer]
    CLI --> Node[Node Service]
    
    subgraph "Core Blockchain Logic"
        Node --> Chain[Blockchain Manager]
        Chain --> State["Account State (Balances/Nonces)"]
        Chain --> Mempool[Pending Transactions]
        Chain --> Store["Storage (Sled DB)"]
    end

    subgraph "Consensus Layer (Pluggable)"
        Chain -.-> Engine[ConsensusEngine Trait]
        Engine --> PoW[Proof of Work]
        Engine --> PoS[Proof of Stake]
        Engine --> PoA[Proof of Authority]
    end

    subgraph "Networking Layer (libp2p)"
        Node --> Swarm[P2P Swarm]
        Swarm --> Gossip["GossipSub (Block/TX Propagation)"]
        Swarm --> Disc["Discovery (Kademlia/mDNS)"]
        Swarm --> PeerMgr[Peer Score & Ban Manager]
    end
```

### Module Responsibilities

| Module | Source File | Description |
| :--- | :--- | :--- |
| **Blockchain** | `src/blockchain.rs` | Orchestrates the chain, validation, and reorg logic. |
| **Block** | `src/block.rs` | `Block` struct, hashing, and signing logic. |
| **Transaction** | `src/transaction.rs` | `Transaction` struct, signature verification, and replay protection. |
| **Account** | `src/account.rs` | State transition logic (balance transfers, nonce increments). |
| **Network** | `src/network/` | P2P stack, protocol messages, and peer reputation. |
| **Consensus** | `src/consensus/` | Implementations of PoW, PoS, and PoA algorithms. |
| **Storage** | `src/storage.rs` | Persistent storage interface using `sled`. |
| **Snapshot** | `src/snapshot.rs` | State snapshotting and pruning for fast sync. |

---

## ⚡ Quick Start

### Prerequisites
- **Rust Toolchain**: `1.70.0+`
- **Dependencies**: `protoc` (Protocol Buffers compiler)

### Build
```bash
git clone https://github.com/rade/budlum-core.git
cd budlum-core
cargo build --release
```

### Running a Node

**1. Proof of Work (Miner)**
```bash
./target/release/budlum-core --consensus pow --difficulty 3 --port 4001
```

**2. Proof of Stake (Validator)**
```bash
./target/release/budlum-core --consensus pos --min-stake 5000 --db-path ./data/pos_node
```

**3. Proof of Authority (Permissioned)**
Create `validators.json`:
```json
{ "validators": ["<validator_pubkey_hex_1>", "<validator_pubkey_hex_2>"] }
```
Run:
```bash
./target/release/budlum-core --consensus poa --validators-file ./validators.json
```

---

## 🔍 Core Components Deep Dive

### 1. Data Structures

The fundamental primitives of the Budlum blockchain are **Blocks** and **Transactions**.

#### Block (`src/block.rs`)
A block contains a header and a body of transactions.
- **`index`**: height of the block (genesis = 0).
- **`hash`**: SHA3-256 hash of the block content.
- **`previous_hash`**: Link to the parent block.
- **`producer`**: Ed25519 Public Key of the node that created the block.
- **`signature`**: Ed25519 Signature of the block hash by the producer.
- **`chain_id`**: Network identifier to prevent cross-chain replay.
- **`transactions`**: A vector of `Transaction` objects.

#### Transaction (`src/transaction.rs`)
A state-changing directive signed by a wallet.
- **`from`/`to`**: Ed25519 Public Keys (Hex).
- **`nonce`**: Sequence number. Must strictly increment (0, 1, 2...) for valid processing.
- **`signature`**: Signs `hash(from, to, amount, fee, nonce, data, chain_id)`.
- **Atomic Execution**: In Budlum, a block is atomic. If *any* transaction is invalid (bad signature, low balance), the *entire block* is rejected.

---

### 2. Consensus Engines

Budlum abstracts consensus into the `ConsensusEngine` trait.

#### Proof of Work (PoW) (`src/consensus/pow.rs`)
- **Algorithm**: Standard SHA3-256 Hashcash.
- **Logic**: `hash(block)` must start with `difficulty` number of zeros.
- **Validation**: Verifiers verify `hash` meets target and structure is correct.

#### Proof of Stake (PoS) (`src/consensus/pos.rs`)
- **Selection**: Deterministic, stake-weighted random selection.
- **Slashing**: Implements **Double-Sign Detection**. If a validator signs two blocks at the same height, their stake is burned.
- **Evidence**: Blocks carry `SlashingEvidence` to enforce penalties.

#### Proof of Authority (PoA) (`src/consensus/poa.rs`)
- **Permissioned**: Only keys in `validators.json` can sign.
- **Round-Robin**: Validators produce blocks in a strict rotation (`height % validator_count`).

---

### 3. Networking Layer

Budlum uses the **libp2p** stack to ensure robust, decentralized peer-to-peer communication.

#### Protocol Messages
Defined in `src/network/protocol.rs`:
- `NewBlock(Block)`: Broadcasts a new block to neighbors.
- `NewTransaction(Transaction)`: Broadcasts a pending transaction.
- `RequestChain` / `Chain(Vec<Block>)`: Used for Initial Block Download (IBD).

#### DoS Protection: Peer Scoring
To prevent spam and attacks, the `PeerManager` (`src/network/peer_manager.rs`) assigns scores:
- **Valid Block**: +1
- **Invalid Block**: -10
- **Oversized Message**: -3
- **Ban Threshold**: -100 (1 Hour Ban)

---

### 4. State Management

Budlum uses an Account-based model (like Ethereum), not UTXO (like Bitcoin).

#### Storage (`src/storage.rs`)
Data is persisted in **sled**, a high-performance embedded database.
- **`BLOCK:{hash}`**: Stores serialized block data.
- **`LAST`**: Stores the hash of the chain tip.
- **`SNAPSHOT:{height}`**: Stores compressed `AccountState`.

#### Snapshots & Pruning (`src/snapshot.rs`)
- **Snapshot Loop**: Every 1000 blocks, the node saves a snapshot of all balances.
- **Pruning**: Blocks older than `2 * max_reorg_depth` (200 blocks) can be pruned to save disk space, as long as a valid snapshot exists ahead of them.

---

### 5. Cryptography & Security

#### Standards
- **Signatures**: **Ed25519** (Schnorr-based). Fast, secure, small keys.
- **Hashing**: **SHA3-256** (Keccak).

#### Domain Separation
We prefix all hashes to prevent context confusion attacks.
- Block Hash Prefix: `BDLM_BLOCK_V1`
- TX Hash Prefix: `BDLM_TX_V1`

#### Chain ID
Every transaction is signed with a specific `chain_id`.
- Mainnet: `1`
- Testnet: `42`
- Devnet: `1337`
This ensures a transaction meant for Testnet cannot be replayed on Mainnet.

---

## 💻 CLI Reference

Usage: `cargo run -- [OPTIONS]`

| Flag | Description | Default |
| :--- | :--- | :--- |
| `--consensus <TYPE>` | `pow` `pos` `poa` | `pow` |
| `--chain-id <ID>` | Network Identifier | `1337` |
| `--port <PORT>` | P2P Listen Port | `4001` |
| `--db-path <PATH>` | Database Directory | `./data/budlum.db` |
| `--difficulty <N>` | Mining Difficulty (PoW) | `2` |
| `--min-stake <AMT>` | Minimum Stake (PoS) | `1000` |
| `--validator-address` | Address to mine/validate for | `None` |
| `--bootstrap <ADDR>` | Peer multiaddr to join | `None` |

---

## 🛠️ Development Guide

### Running Tests
Budlum has extensive unit and integration tests (69+ tests).
```bash
cargo test
```

**Key Test Suites:**
- `integration_tests`: Simulates full node interactions.
- `consensus::pos::tests`: Validates slashing and staking logic.
- `network::peer_manager::tests`: Validates banning logic.

### Code Style
- Format: `cargo fmt`
- Lint: `cargo clippy`

---

## 📄 License
MIT License. Copyright (c) 2026 The Budlum Developers.
