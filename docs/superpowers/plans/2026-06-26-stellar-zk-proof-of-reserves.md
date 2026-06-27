# Stellar ZK Proof-of-Reserves Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let an entity that custodies user USDC prove on Stellar that its real on-chain reserves cover the sum of all user liabilities (`R ≥ Σ balanceᵢ`, every balance valid) without revealing any individual balance, with each user able to self-verify Merkle inclusion.

**Architecture:** A RISC Zero zkVM guest builds a SHA-256 Merkle-sum tree over a private balance set, range-checks each balance, sums them with overflow checks, and commits a 52-byte public journal `(root M ‖ total L ‖ snapshot T ‖ count N)`. The host wraps the proof to Groth16 and submits `(seal, M, L, T, N)` to a Soroban contract. The contract reconstructs the journal digest, verifies the receipt through the deployed NethermindEth RISC Zero router, reads the issuer's **live** USDC balance via the asset's SAC, asserts `R ≥ L`, and stores a public attestation. A pure-Rust `por-core` crate holds the tree/journal logic and is shared by the guest, host, and an off-chain inclusion verifier.

**Tech Stack:** Rust; RISC Zero zkVM `risc0-zkvm` 3.0.x + `risc0-build` + `risc0-ethereum-contracts` (`encode_seal`); `sha2` 0.10; `serde`; Soroban `soroban-sdk` 25.1.0 (edition 2024); NethermindEth `stellar-risc0-verifier` (router + groth16-verifier, self-deployed); Stellar CLI (`stellar`); a static frontend using `@stellar/stellar-sdk`.

## Global Constraints

- **Repo artifacts in English** — README, specs, code comments, commit messages. (Chat with the user is in Chinese; the repo is judged by the English-speaking Stellar team.)
- **soroban-sdk = "25.1.0"**, contract crate **edition = "2024"** — must match the deployed NethermindEth verifier's ABI/SDK exactly. Do not bump without re-checking the verifier repo.
- **RISC Zero 3.0.x** for `risc0-zkvm`, `risc0-build`, `risc0-ethereum-contracts` — the proof's RISC Zero version must match the deployed verifier's, or on-chain verification fails.
- **Groth16 proving needs x86_64 + Docker + ≈16 GB RAM.** The dev box is WSL2 with 7.8 GB. The proving location is resolved in Task 0.1 (default: raise the WSL2 memory cap; fallbacks: Bonsai or a cloud x86 VM). Every task that runs the prover assumes that decision is in place.
- **Never commit secrets** (`.env`, `*.key`, `*.pem`, `identity.toml`) — already covered by `.gitignore`.
- **Journal byte layout is a hard cross-boundary contract:** little-endian, field order `root(32) ‖ total:u64(8) ‖ snapshot:u64(8) ‖ count:u32(4)` = 52 bytes. The guest's `encode_journal`, the host's `decode_journal`, and the contract's in-Wasm reconstruction must all agree byte-for-byte.
- **USDC has 7 decimals:** balances and reserves are in stroops, `1 USDC = 10_000_000`. Liabilities `L` are summed as `u64`; reserves `R` are read as `i128`.
- **Hashing is SHA-256 everywhere** (guest, off-chain verifier). The contract never recomputes the tree — `M` is opaque to it — so no ZK-friendly hash is needed. Leaves/nodes/padding are domain-separated by a prefix byte (`0x00` leaf, `0x01` node, `0x02` padding).

---

## Repository layout (target)

```
Stellar-Hacks/
├── Cargo.toml                      # HOST workspace: por-core, methods, host, verifier-cli
├── por-core/                       # shared pure-Rust: Merkle-sum tree, inclusion, journal codec
│   ├── Cargo.toml
│   └── src/lib.rs
├── methods/                        # RISC Zero build glue
│   ├── Cargo.toml
│   ├── build.rs
│   ├── src/lib.rs
│   └── guest/                      # detached workspace; builds for riscv32im
│       ├── Cargo.toml
│       └── src/main.rs
├── host/                           # prover CLI: dataset → Groth16 receipt → proof.json + inclusion proofs
│   ├── Cargo.toml
│   └── src/main.rs
├── verifier-cli/                   # off-chain user inclusion verifier
│   ├── Cargo.toml
│   └── src/main.rs
├── contracts/
│   └── attestation/                # OWN workspace: soroban-sdk 25.1.0, edition 2024
│       ├── Cargo.toml
│       └── src/{lib.rs,test.rs}
├── frontend/                       # demo dashboard (functional first; visuals later via open-design)
├── data/
│   └── mock-balances.json
└── docs/
```

The RISC Zero host workspace, the guest (riscv32im target), and the Soroban contract (wasm32 target, edition 2024) are **three separate Cargo workspaces** to avoid resolver/edition/profile conflicts between the RISC Zero and Soroban dependency trees.

---

## Phase 0 — Toolchain & end-to-end integration spike

**Why this phase exists / gate:** The single biggest risk is the integration seam — self-deploying the NethermindEth verifier, producing a Groth16 proof on a RAM-constrained box, and matching the RISC Zero version end-to-end. Phase 0 proves the *entire pipeline works with a trivial guest* before any real PoR logic is written. Do not start Phase 1 until Task 0.4 verifies a real proof on-chain.

### Task 0.0: Repository prerequisites & commit the design spec

**Files:**
- Modify: (none — uses existing `.gitignore`, `docs/superpowers/specs/...`)

- [ ] **Step 1: Confirm git identity is set** (the spec commit was blocked on this earlier)

Run: `git config user.name && git config user.email`
Expected: prints a name and email. If either is empty, ask the user to run (in the session):
`! git config user.name "Their Name" && git config user.email "their@email"`
Do **not** set git config yourself.

- [ ] **Step 2: Initialize the repo if needed and commit the spec**

```bash
git rev-parse --is-inside-work-tree 2>/dev/null || git init
git add docs/superpowers/specs/2026-06-26-stellar-zk-proof-of-reserves-design.md \
        docs/superpowers/plans/2026-06-26-stellar-zk-proof-of-reserves.md .gitignore
git commit -m "docs: add ZK proof-of-reserves design spec and implementation plan"
```
Expected: a commit is created. Verify with `git log --oneline -1`.

### Task 0.1: Resolve the proving location & install toolchains

**Decision — where Groth16 proving runs.** Pick one (default = A):

- **A. Raise the WSL2 memory cap (recommended; free, local, no external dependency).** Works only if the Windows host has ≈20 GB+ physical RAM. On Windows, create/edit `C:\Users\<you>\.wslconfig`:
  ```ini
  [wsl2]
  memory=20GB
  swap=16GB
  processors=8
  ```
  Then in PowerShell `wsl --shutdown`, reopen the WSL terminal.
- **B. Bonsai (remote proving; zero local RAM).** Request an API key, then export `BONSAI_API_URL` and `BONSAI_API_KEY`. `default_prover()` auto-selects Bonsai when both are set.
- **C. Cloud x86_64 VM with Docker + 32 GB.** Prove there, copy `proof.json` back.

- [ ] **Step 1: Apply the chosen proving option** (default A). For A, after restart verify:

Run: `free -h`
Expected: `Mem:` total shows ~20 GB (not 7.8 GB). If the host lacks the RAM, switch to option B or C before proceeding.

- [ ] **Step 2: Install Rust targets, Docker, RISC Zero, and the Stellar CLI**

```bash
# Rust + targets
rustup toolchain install stable
rustup target add wasm32v1-none

# Docker (required for the STARK→Groth16 step). Verify the daemon runs:
docker run --rm hello-world

# RISC Zero toolchain
curl -L https://risczero.com/install | bash
rzup install
rzup install risc0-groth16          # the Groth16 prover component (x86_64)

# Stellar CLI
cargo install stellar-cli --locked
```

- [ ] **Step 3: Verify versions**

Run:
```bash
rzup show && cargo risczero --version && stellar --version && docker --version
```
Expected: `cargo risczero` reports a 3.0.x toolchain; `stellar` reports a recent CLI (Protocol 23+). Record the exact RISC Zero version — it must match what the verifier targets (`risc0-zkvm = "^3.0"`).

- [ ] **Step 4: Create a funded testnet identity**

```bash
stellar keys generate issuer --network testnet
stellar keys fund issuer --network testnet
stellar keys address issuer
```
Expected: prints a `G...` address. (`identity.toml`/keys stay out of git via `.gitignore`.)

### Task 0.2: Scaffold the host workspace + a trivial proving spike

**Files:**
- Create: `Cargo.toml`, `methods/Cargo.toml`, `methods/build.rs`, `methods/src/lib.rs`, `methods/guest/Cargo.toml`, `methods/guest/src/main.rs`, `host/Cargo.toml`, `host/src/main.rs`

**Interfaces:**
- Produces: a `host` binary that prints `seal`, `image_id`, `journal_digest` as hex — the exact three values the verifier consumes.

- [ ] **Step 1: Host workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["por-core", "methods", "host", "verifier-cli"]

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
risc0-zkvm = "3.0"
risc0-build = "3.0"
risc0-ethereum-contracts = "3.0"
sha2 = "0.10"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
hex = "0.4"
anyhow = "1.0"
clap = { version = "4", features = ["derive"] }
```
(`por-core` and `verifier-cli` are added to `members` now; their crates are created in Phase 1 / Phase 6. If a member directory does not yet exist when you build `host`, temporarily trim `members` to `["methods","host"]` for this task and restore it in Task 1.1.)

- [ ] **Step 2: `methods/Cargo.toml`, `methods/build.rs`, `methods/src/lib.rs`**

`methods/Cargo.toml`:
```toml
[package]
name = "methods"
version = "0.1.0"
edition = "2021"

[build-dependencies]
risc0-build = { workspace = true }

[package.metadata.risc0]
methods = ["guest"]
```
`methods/build.rs`:
```rust
fn main() {
    risc0_build::embed_methods();
}
```
`methods/src/lib.rs`:
```rust
include!(concat!(env!("OUT_DIR"), "/methods.rs"));
```

- [ ] **Step 3: Guest crate (detached workspace) — trivial spike guest**

`methods/guest/Cargo.toml`:
```toml
[package]
name = "por-guest"
version = "0.1.0"
edition = "2021"

[workspace]

[[bin]]
name = "por-guest"
path = "src/main.rs"

[dependencies]
risc0-zkvm = { version = "3.0", default-features = false, features = ["std"] }
```
`methods/guest/src/main.rs` (trivial: read a u64, commit it back as raw bytes):
```rust
#![no_main]
risc0_zkvm::guest::entry!(main);
use risc0_zkvm::guest::env;

fn main() {
    let x: u64 = env::read();
    env::commit_slice(&x.to_le_bytes());
}
```

- [ ] **Step 4: Host spike — prove to Groth16 and print the three values**

`host/Cargo.toml`:
```toml
[package]
name = "host"
version = "0.1.0"
edition = "2021"

[dependencies]
methods = { path = "../methods" }
risc0-zkvm = { workspace = true }
risc0-ethereum-contracts = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }
anyhow = { workspace = true }
```
`host/src/main.rs`:
```rust
use anyhow::Result;
use methods::{POR_GUEST_ELF, POR_GUEST_ID};
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts};
use sha2::{Digest, Sha256};

fn main() -> Result<()> {
    let env = ExecutorEnv::builder().write(&7u64)?.build()?;
    let prove_info = default_prover().prove_with_opts(env, POR_GUEST_ELF, &ProverOpts::groth16())?;
    let receipt = prove_info.receipt;

    let seal = encode_seal(&receipt)?;
    let journal_digest: [u8; 32] = Sha256::digest(&receipt.journal.bytes).into();
    let image_id: [u8; 32] = POR_GUEST_ID;

    println!("seal     {}", hex::encode(&seal));
    println!("image_id {}", hex::encode(image_id));
    println!("journal  {}", hex::encode(journal_digest));
    Ok(())
}
```

- [ ] **Step 5: Run the spike under dev mode first (fast, no Groth16) to shake out wiring**

Run: `RISC0_DEV_MODE=1 cargo run -p host`
Expected: prints three hex lines (dev-mode seal is a stub — that's fine; this only checks the build/wiring).

- [ ] **Step 6: Run the real Groth16 prove** (uses Docker; slow, RAM-heavy)

Run: `cargo run -p host --release`
Expected: after several minutes, three hex lines with a **real** `seal` (long) and a 32-byte `image_id`/`journal`. If this OOMs, the proving-location decision (Task 0.1) needs option B or C.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml methods host
git commit -m "feat: RISC Zero host+guest scaffold with Groth16 proving spike"
```

### Task 0.3: Self-deploy the NethermindEth verifier (local network first)

**Files:** none in our repo (external tooling). Record the deployed router id in `deployment.toml` notes.

- [ ] **Step 1: Clone and build the verifier repo**

```bash
git clone https://github.com/NethermindEth/stellar-risc0-verifier ../stellar-risc0-verifier
cd ../stellar-risc0-verifier
cargo build --release
```
Expected: workspace builds (soroban-sdk 25.1.0, edition 2024).

- [ ] **Step 2: Start a local network and deploy router + verifier**

```bash
stellar container start local
stellar keys generate foo --network local && stellar keys fund foo --network local

./scripts/manage.sh deploy-router   -n local -a foo --min-delay 0
./scripts/manage.sh deploy-verifier -n local -a foo

SELECTOR=$(python3 ./scripts/toml_helper.py read deployment.toml chains.stellar-local.verifiers.0.selector)
./scripts/manage.sh schedule-add-verifier -n local -a foo --selector "$SELECTOR"
./scripts/manage.sh execute-add-verifier  -n local -a foo --selector "$SELECTOR"
./scripts/manage.sh status -n local
```
Expected: `status` prints a router contract id. Save it as `ROUTER_LOCAL`.

### Task 0.4: Verify the spike proof on-chain (the de-risk gate)

- [ ] **Step 1: Write the spike proof to `proof.txt`**

Temporarily adapt `host/src/main.rs` Step 4 to also `std::fs::write("proof.txt", format!("{}\n{}\n{}\n", hex::encode(&seal), hex::encode(image_id), hex::encode(journal_digest)))?;` then:
Run: `cargo run -p host --release`

- [ ] **Step 2: Invoke the router's `verify` with the spike values**

```bash
cd ../stellar-risc0-verifier
SEAL_HEX=$(sed -n '1p' ../Stellar-Hacks/proof.txt)
IMAGE_ID_HEX=$(sed -n '2p' ../Stellar-Hacks/proof.txt)
JOURNAL_DIGEST_HEX=$(sed -n '3p' ../Stellar-Hacks/proof.txt)
stellar contract invoke --send=no --network local --source foo --id "$ROUTER_LOCAL" \
  -- verify --seal "$SEAL_HEX" --image_id "$IMAGE_ID_HEX" --journal "$JOURNAL_DIGEST_HEX"
```
Expected: simulation **succeeds** (no error). This proves the full RISC Zero ↔ Soroban seam end-to-end.

- [ ] **Step 3: Repeat the deploy + verify on `testnet`** (the network the demo will use)

```bash
./scripts/manage.sh deploy-router   -n testnet -a issuer --min-delay 0
./scripts/manage.sh deploy-verifier -n testnet -a issuer
SELECTOR=$(python3 ./scripts/toml_helper.py read deployment.toml chains.stellar-testnet.verifiers.0.selector)
./scripts/manage.sh schedule-add-verifier -n testnet -a issuer --selector "$SELECTOR"
./scripts/manage.sh execute-add-verifier  -n testnet -a issuer --selector "$SELECTOR"
./scripts/manage.sh status -n testnet
```
Expected: a testnet router id. Save it as `ROUTER_TESTNET`. Re-run Step 2 against `--network testnet` to confirm.

> **GATE:** If Tasks 0.1–0.4 pass, the integration risk is retired and the rest of the plan is "ordinary" Rust. If on-chain verify fails with a version error, align the local RISC Zero version (Task 0.1 Step 3) to the verifier's before continuing.

---

## Phase 1 — `por-core`: Merkle-sum tree, inclusion, journal codec

**Why / gate:** This is the cryptographic heart and is pure, deterministic Rust — fully unit-testable off-zkVM. Build it test-first so the guest (Phase 2) is a thin wrapper. The same crate is reused by the off-chain verifier (Phase 6) so its hashing exactly matches the prover's.

### Task 1.1: `por-core` skeleton + types + domain-separated hashing

**Files:**
- Create: `por-core/Cargo.toml`, `por-core/src/lib.rs`
- Modify: `Cargo.toml` (restore `por-core` to `members` if trimmed in Task 0.2)

**Interfaces:**
- Produces: `Account { id: u64, balance: u64 }`; `PorError`; in-crate `hash_leaf`, `hash_node`, `hash_padding`.

- [ ] **Step 1: `por-core/Cargo.toml`**

```toml
[package]
name = "por-core"
version = "0.1.0"
edition = "2021"

[dependencies]
sha2 = { workspace = true }
serde = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
```

- [ ] **Step 2: Write the failing test for leaf/node domain separation** (`por-core/src/lib.rs`, `#[cfg(test)]`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaf_node_padding_are_domain_separated() {
        // Same 32 bytes + sums must hash differently as leaf vs node vs padding.
        let a = hash_leaf(1, 100);
        let b = hash_padding(0);
        assert_ne!(a, b);
        let n = hash_node(&a, 100, &b, 0);
        assert_ne!(n, a);
        assert_ne!(n, b);
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p por-core leaf_node_padding -- --nocapture`
Expected: FAIL — `hash_leaf`/`hash_node`/`hash_padding` not found.

- [ ] **Step 4: Implement types + hashing** (top of `por-core/src/lib.rs`)

```rust
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: u64,
    pub balance: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PorError {
    Empty,
    Overflow,
    IndexOutOfRange,
    BadJournal,
}

const TAG_LEAF: u8 = 0x00;
const TAG_NODE: u8 = 0x01;
const TAG_PADDING: u8 = 0x02;

pub(crate) fn hash_leaf(id: u64, balance: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([TAG_LEAF]);
    h.update(id.to_le_bytes());
    h.update(balance.to_le_bytes());
    h.finalize().into()
}

pub(crate) fn hash_padding(index: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([TAG_PADDING]);
    h.update(index.to_le_bytes());
    h.finalize().into()
}

pub(crate) fn hash_node(l: &[u8; 32], l_sum: u64, r: &[u8; 32], r_sum: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([TAG_NODE]);
    h.update(l);
    h.update(r);
    h.update(l_sum.to_le_bytes());
    h.update(r_sum.to_le_bytes());
    h.finalize().into()
}
```

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p por-core leaf_node_padding`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add por-core Cargo.toml
git commit -m "feat(por-core): account type and domain-separated SHA-256 hashing"
```

### Task 1.2: Build the Merkle-sum tree (root, total, overflow + range checks)

**Files:**
- Modify: `por-core/src/lib.rs`

**Interfaces:**
- Produces: `MerkleSumTree::build(&[Account]) -> Result<MerkleSumTree, PorError>`, `.root() -> [u8;32]`, `.total() -> u64`, `.count() -> usize`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn build_sums_and_is_deterministic() {
    let accounts = vec![
        Account { id: 1, balance: 100 },
        Account { id: 2, balance: 250 },
        Account { id: 3, balance: 0 },
    ];
    let t = MerkleSumTree::build(&accounts).unwrap();
    assert_eq!(t.total(), 350);
    assert_eq!(t.count(), 3);
    // determinism
    let t2 = MerkleSumTree::build(&accounts).unwrap();
    assert_eq!(t.root(), t2.root());
}

#[test]
fn build_rejects_empty() {
    assert_eq!(MerkleSumTree::build(&[]).unwrap_err(), PorError::Empty);
}

#[test]
fn build_rejects_overflow() {
    let accounts = vec![
        Account { id: 1, balance: u64::MAX },
        Account { id: 2, balance: 1 },
    ];
    assert_eq!(MerkleSumTree::build(&accounts).unwrap_err(), PorError::Overflow);
}

#[test]
fn single_account_root_is_leaf_chained_to_self_pow2() {
    // N=1 pads to 1 leaf (already a power of two): root == leaf hash, total == balance.
    let t = MerkleSumTree::build(&[Account { id: 9, balance: 42 }]).unwrap();
    assert_eq!(t.total(), 42);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p por-core build_`
Expected: FAIL — `MerkleSumTree` not found.

- [ ] **Step 3: Implement the tree**

```rust
#[derive(Clone, Debug)]
struct Node {
    hash: [u8; 32],
    sum: u64,
}

#[derive(Clone, Debug)]
pub struct MerkleSumTree {
    accounts: Vec<Account>,
    levels: Vec<Vec<Node>>, // levels[0] = padded leaves; last = [root]
}

impl MerkleSumTree {
    pub fn build(accounts: &[Account]) -> Result<MerkleSumTree, PorError> {
        if accounts.is_empty() {
            return Err(PorError::Empty);
        }
        let mut total: u64 = 0;
        for a in accounts {
            total = total.checked_add(a.balance).ok_or(PorError::Overflow)?;
        }

        let n = accounts.len();
        let padded = n.next_power_of_two();
        let mut leaves: Vec<Node> = Vec::with_capacity(padded);
        for a in accounts {
            leaves.push(Node { hash: hash_leaf(a.id, a.balance), sum: a.balance });
        }
        for i in n..padded {
            leaves.push(Node { hash: hash_padding(i as u64), sum: 0 });
        }

        let mut levels = vec![leaves];
        while levels.last().unwrap().len() > 1 {
            let cur = levels.last().unwrap();
            let mut next = Vec::with_capacity(cur.len() / 2);
            for pair in cur.chunks(2) {
                let sum = pair[0].sum + pair[1].sum; // safe: bounded by `total` (already u64-checked)
                let hash = hash_node(&pair[0].hash, pair[0].sum, &pair[1].hash, pair[1].sum);
                next.push(Node { hash, sum });
            }
            levels.push(next);
        }

        Ok(MerkleSumTree { accounts: accounts.to_vec(), levels })
    }

    pub fn root(&self) -> [u8; 32] {
        self.levels.last().unwrap()[0].hash
    }

    pub fn total(&self) -> u64 {
        self.levels.last().unwrap()[0].sum
    }

    pub fn count(&self) -> usize {
        self.accounts.len()
    }
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p por-core build_ single_account`
Expected: PASS (all four).

- [ ] **Step 5: Commit**

```bash
git add por-core/src/lib.rs
git commit -m "feat(por-core): Merkle-sum tree build with range + overflow checks"
```

### Task 1.3: Inclusion proofs (generate + verify)

**Files:**
- Modify: `por-core/src/lib.rs`

**Interfaces:**
- Produces: `Sibling { hash:[u8;32], sum:u64, is_left:bool }`, `InclusionProof { id:u64, balance:u64, index:usize, siblings:Vec<Sibling> }`, `MerkleSumTree::inclusion_proof(index) -> Result<InclusionProof, PorError>`, free fn `verify_inclusion(&InclusionProof, &[u8;32]) -> bool`. All proof types derive `Serialize, Deserialize`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn inclusion_roundtrip_valid() {
    let accounts = vec![
        Account { id: 10, balance: 100 },
        Account { id: 20, balance: 250 },
        Account { id: 30, balance: 5 },
        Account { id: 40, balance: 0 },
        Account { id: 50, balance: 700 },
    ];
    let t = MerkleSumTree::build(&accounts).unwrap();
    let root = t.root();
    for i in 0..accounts.len() {
        let p = t.inclusion_proof(i).unwrap();
        assert_eq!(p.id, accounts[i].id);
        assert_eq!(p.balance, accounts[i].balance);
        assert!(verify_inclusion(&p, &root), "proof {i} should verify");
    }
}

#[test]
fn inclusion_rejects_tampered_balance() {
    let accounts = vec![Account { id: 1, balance: 100 }, Account { id: 2, balance: 200 }];
    let t = MerkleSumTree::build(&accounts).unwrap();
    let root = t.root();
    let mut p = t.inclusion_proof(0).unwrap();
    p.balance += 1; // lie
    assert!(!verify_inclusion(&p, &root));
}

#[test]
fn inclusion_rejects_wrong_root() {
    let accounts = vec![Account { id: 1, balance: 100 }, Account { id: 2, balance: 200 }];
    let t = MerkleSumTree::build(&accounts).unwrap();
    let p = t.inclusion_proof(1).unwrap();
    assert!(!verify_inclusion(&p, &[0u8; 32]));
}

#[test]
fn inclusion_index_out_of_range() {
    let t = MerkleSumTree::build(&[Account { id: 1, balance: 1 }]).unwrap();
    assert_eq!(t.inclusion_proof(5).unwrap_err(), PorError::IndexOutOfRange);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p por-core inclusion_`
Expected: FAIL — `inclusion_proof`/`verify_inclusion`/`Sibling`/`InclusionProof` not found.

- [ ] **Step 3: Implement generation + verification**

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sibling {
    pub hash: [u8; 32],
    pub sum: u64,
    pub is_left: bool, // true if the sibling sits on the LEFT of the current node
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InclusionProof {
    pub id: u64,
    pub balance: u64,
    pub index: usize,
    pub siblings: Vec<Sibling>,
}

impl MerkleSumTree {
    pub fn inclusion_proof(&self, index: usize) -> Result<InclusionProof, PorError> {
        if index >= self.accounts.len() {
            return Err(PorError::IndexOutOfRange);
        }
        let mut siblings = Vec::new();
        let mut idx = index;
        for level in &self.levels[..self.levels.len() - 1] {
            let sib_idx = idx ^ 1;
            let sib = &level[sib_idx];
            siblings.push(Sibling { hash: sib.hash, sum: sib.sum, is_left: sib_idx < idx });
            idx /= 2;
        }
        let acct = self.accounts[index];
        Ok(InclusionProof { id: acct.id, balance: acct.balance, index, siblings })
    }
}

pub fn verify_inclusion(proof: &InclusionProof, expected_root: &[u8; 32]) -> bool {
    let mut hash = hash_leaf(proof.id, proof.balance);
    let mut sum = proof.balance;
    for s in &proof.siblings {
        if s.is_left {
            hash = hash_node(&s.hash, s.sum, &hash, sum);
        } else {
            hash = hash_node(&hash, sum, &s.hash, s.sum);
        }
        sum = match sum.checked_add(s.sum) {
            Some(v) => v,
            None => return false,
        };
    }
    &hash == expected_root
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p por-core inclusion_`
Expected: PASS (all four).

- [ ] **Step 5: Commit**

```bash
git add por-core/src/lib.rs
git commit -m "feat(por-core): Merkle inclusion proof generation and verification"
```

### Task 1.4: Journal codec (the 52-byte cross-boundary layout)

**Files:**
- Modify: `por-core/src/lib.rs`

**Interfaces:**
- Produces: `JOURNAL_LEN: usize` (=52); `Journal { root:[u8;32], total:u64, snapshot:u64, count:u32 }`; `encode_journal(&Journal) -> [u8; JOURNAL_LEN]`; `decode_journal(&[u8]) -> Result<Journal, PorError>`.

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn journal_encode_layout_is_exact() {
    let j = Journal { root: [0xAB; 32], total: 0x1122334455667788, snapshot: 0x00000000DEADBEEF, count: 0x01020304 };
    let b = encode_journal(&j);
    assert_eq!(b.len(), 52);
    assert_eq!(&b[0..32], &[0xAB; 32]);                                  // root
    assert_eq!(&b[32..40], &0x1122334455667788u64.to_le_bytes());       // total LE
    assert_eq!(&b[40..48], &0x00000000DEADBEEFu64.to_le_bytes());       // snapshot LE
    assert_eq!(&b[48..52], &0x01020304u32.to_le_bytes());               // count LE
}

#[test]
fn journal_roundtrip() {
    let j = Journal { root: [7; 32], total: 999_999, snapshot: 1_700_000_000, count: 12345 };
    let decoded = decode_journal(&encode_journal(&j)).unwrap();
    assert_eq!(decoded, j);
}

#[test]
fn journal_decode_rejects_wrong_length() {
    assert_eq!(decode_journal(&[0u8; 51]).unwrap_err(), PorError::BadJournal);
}
```
Add `#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]` on `Journal` so the roundtrip equality works.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p por-core journal_`
Expected: FAIL — `Journal`/`encode_journal`/`decode_journal` not found.

- [ ] **Step 3: Implement the codec**

```rust
pub const JOURNAL_LEN: usize = 32 + 8 + 8 + 4; // 52

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    pub root: [u8; 32],
    pub total: u64,
    pub snapshot: u64,
    pub count: u32,
}

pub fn encode_journal(j: &Journal) -> [u8; JOURNAL_LEN] {
    let mut out = [0u8; JOURNAL_LEN];
    out[0..32].copy_from_slice(&j.root);
    out[32..40].copy_from_slice(&j.total.to_le_bytes());
    out[40..48].copy_from_slice(&j.snapshot.to_le_bytes());
    out[48..52].copy_from_slice(&j.count.to_le_bytes());
    out
}

pub fn decode_journal(bytes: &[u8]) -> Result<Journal, PorError> {
    if bytes.len() != JOURNAL_LEN {
        return Err(PorError::BadJournal);
    }
    let mut root = [0u8; 32];
    root.copy_from_slice(&bytes[0..32]);
    let total = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
    let snapshot = u64::from_le_bytes(bytes[40..48].try_into().unwrap());
    let count = u32::from_le_bytes(bytes[48..52].try_into().unwrap());
    Ok(Journal { root, total, snapshot, count })
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p por-core`
Expected: PASS (entire `por-core` suite green).

- [ ] **Step 5: Commit**

```bash
git add por-core/src/lib.rs
git commit -m "feat(por-core): 52-byte journal codec (root|total|snapshot|count, LE)"
```

---

## Phase 2 — RISC Zero guest

**Why / gate:** Replace the trivial spike guest with the real PoR computation. The guest stays a thin wrapper over `por-core`; correctness is already covered by Phase 1 tests, so here we test only the guest↔host I/O contract (input read + journal commit) via the executor in dev mode.

### Task 2.1: Real guest + executor (dev-mode) round-trip test

**Files:**
- Modify: `methods/guest/Cargo.toml`, `methods/guest/src/main.rs`
- Create: `host/tests/guest_journal.rs`

**Interfaces:**
- Consumes: `por-core` (`Account`, `MerkleSumTree`, `Journal`, `encode_journal`, `decode_journal`).
- Produces: a guest that reads `Vec<Account>` then `u64` snapshot, and commits `encode_journal(...)` (52 bytes).

- [ ] **Step 1: Point the guest at `por-core`**

`methods/guest/Cargo.toml` dependencies:
```toml
[dependencies]
risc0-zkvm = { version = "3.0", default-features = false, features = ["std"] }
por-core = { path = "../../por-core" }
```
(Optional, perf only — accelerated SHA-256 in the zkVM. If the tag fails to resolve, omit this block; it does not affect correctness.)
```toml
[patch.crates-io]
sha2 = { git = "https://github.com/risc0/RustCrypto-hashes", tag = "sha2-v0.10.8-risczero.0" }
```

- [ ] **Step 2: Real guest**

`methods/guest/src/main.rs`:
```rust
#![no_main]
risc0_zkvm::guest::entry!(main);
use risc0_zkvm::guest::env;
use por_core::{encode_journal, Account, Journal, MerkleSumTree};

fn main() {
    let accounts: Vec<Account> = env::read();
    let snapshot: u64 = env::read();

    let tree = MerkleSumTree::build(&accounts).expect("invalid balance set");
    let journal = Journal {
        root: tree.root(),
        total: tree.total(),
        snapshot,
        count: accounts.len() as u32,
    };
    env::commit_slice(&encode_journal(&journal));
}
```

- [ ] **Step 3: Write the failing host-side executor test**

`host/tests/guest_journal.rs`:
```rust
use methods::POR_GUEST_ELF;
use por_core::{decode_journal, Account, MerkleSumTree};
use risc0_zkvm::{default_executor, ExecutorEnv};

#[test]
fn guest_commits_expected_journal() {
    let accounts = vec![
        Account { id: 1, balance: 100 },
        Account { id: 2, balance: 250 },
        Account { id: 3, balance: 0 },
    ];
    let snapshot: u64 = 1_700_000_000;

    let env = ExecutorEnv::builder()
        .write(&accounts).unwrap()
        .write(&snapshot).unwrap()
        .build().unwrap();

    // Executor runs the guest without proving — fast, no Docker/RAM.
    let session = default_executor().execute(env, POR_GUEST_ELF).unwrap();
    let journal = decode_journal(&session.journal.bytes).unwrap();

    let expected = MerkleSumTree::build(&accounts).unwrap();
    assert_eq!(journal.root, expected.root());
    assert_eq!(journal.total, 350);
    assert_eq!(journal.snapshot, snapshot);
    assert_eq!(journal.count, 3);
}
```
Add `[dev-dependencies] por-core = { path = "../por-core" }` and `risc0-zkvm = { workspace = true }` to `host/Cargo.toml` if not already present (they are, as normal deps — `por-core` must be added).

- [ ] **Step 4: Run to verify it fails, then passes after build**

Run: `cargo test -p host --test guest_journal`
Expected: first run rebuilds the guest ELF; test PASSES (journal decodes to the expected root/total/snapshot/count). If it fails to find `por_core` in the test, add it to `host` deps.

- [ ] **Step 5: Commit**

```bash
git add methods/guest host
git commit -m "feat(guest): real Merkle-sum PoR guest + executor journal round-trip test"
```

---

## Phase 3 — Host prover CLI

**Why / gate:** Turn the spike `host` into a real CLI: load a mock dataset, prove to Groth16, and emit everything the contract and users need — `proof.json` (seal + journal fields + digest + image id) and one inclusion proof file per user.

### Task 3.1: Mock dataset + CLI that proves and writes artifacts

**Files:**
- Create: `data/mock-balances.json`, `host/src/main.rs` (replace spike)
- Modify: `host/Cargo.toml` (add `por-core`, `serde_json`, `clap`, `serde`)

**Interfaces:**
- Produces: `out/proof.json` `{ seal_hex, image_id_hex, journal_digest_hex, root_hex, total, snapshot, count }`; `out/inclusion/<id>.json` (serialized `InclusionProof`).

- [ ] **Step 1: Mock dataset**

`data/mock-balances.json`:
```json
[
  { "id": 1001, "balance": 125000000 },
  { "id": 1002, "balance": 47500000 },
  { "id": 1003, "balance": 980000000 },
  { "id": 1004, "balance": 0 },
  { "id": 1005, "balance": 3300000000 },
  { "id": 1006, "balance": 1500000 },
  { "id": 1007, "balance": 12750000 }
]
```
(Balances in stroops; total = 4 466 750 000 stroops = 446.675 USDC.)

- [ ] **Step 2: Host `Cargo.toml` deps**

```toml
[dependencies]
methods = { path = "../methods" }
por-core = { path = "../por-core" }
risc0-zkvm = { workspace = true }
risc0-ethereum-contracts = { workspace = true }
sha2 = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
hex = { workspace = true }
anyhow = { workspace = true }
clap = { workspace = true }
```

- [ ] **Step 3: CLI implementation**

`host/src/main.rs`:
```rust
use anyhow::{Context, Result};
use clap::Parser;
use methods::{POR_GUEST_ELF, POR_GUEST_ID};
use por_core::{decode_journal, Account, MerkleSumTree};
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    /// Path to the mock balances JSON.
    #[arg(long, default_value = "data/mock-balances.json")]
    balances: PathBuf,
    /// Snapshot timestamp T (unix seconds).
    #[arg(long, default_value_t = 1_700_000_000)]
    snapshot: u64,
    /// Output directory.
    #[arg(long, default_value = "out")]
    out: PathBuf,
}

#[derive(Serialize)]
struct ProofFile {
    seal_hex: String,
    image_id_hex: String,
    journal_digest_hex: String,
    root_hex: String,
    total: u64,
    snapshot: u64,
    count: u32,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let accounts: Vec<Account> =
        serde_json::from_slice(&fs::read(&args.balances)?).context("reading balances")?;

    // Build the tree off-zkVM too, so we can emit inclusion proofs.
    let tree = MerkleSumTree::build(&accounts).context("building tree")?;

    let env = ExecutorEnv::builder()
        .write(&accounts)?
        .write(&args.snapshot)?
        .build()?;
    let receipt = default_prover()
        .prove_with_opts(env, POR_GUEST_ELF, &ProverOpts::groth16())?
        .receipt;

    let seal = encode_seal(&receipt)?;
    let journal_digest: [u8; 32] = Sha256::digest(&receipt.journal.bytes).into();
    let image_id: [u8; 32] = POR_GUEST_ID;
    let journal = decode_journal(&receipt.journal.bytes)?;

    fs::create_dir_all(args.out.join("inclusion"))?;
    let proof = ProofFile {
        seal_hex: hex::encode(&seal),
        image_id_hex: hex::encode(image_id),
        journal_digest_hex: hex::encode(journal_digest),
        root_hex: hex::encode(journal.root),
        total: journal.total,
        snapshot: journal.snapshot,
        count: journal.count,
    };
    fs::write(args.out.join("proof.json"), serde_json::to_vec_pretty(&proof)?)?;

    for i in 0..accounts.len() {
        let p = tree.inclusion_proof(i)?;
        fs::write(
            args.out.join("inclusion").join(format!("{}.json", p.id)),
            serde_json::to_vec_pretty(&p)?,
        )?;
    }

    println!("wrote {}/proof.json  (total={} stroops, count={})", args.out.display(), proof.total, proof.count);
    Ok(())
}
```

- [ ] **Step 4: Smoke-run in dev mode** (fast; verifies dataset load + artifact writing)

Run: `RISC0_DEV_MODE=1 cargo run -p host --release`
Expected: prints `wrote out/proof.json (total=4466750000 stroops, count=7)`; `out/inclusion/` has 7 files. (Dev-mode `seal` is a stub; real seal comes next.)

- [ ] **Step 5: Real Groth16 run**

Run: `cargo run -p host --release`
Expected: same output, with a real `seal_hex` in `out/proof.json` (long hex). This is the artifact the contract will consume.

- [ ] **Step 6: Commit**

```bash
git add host data
git commit -m "feat(host): prover CLI — dataset to Groth16 proof.json + per-user inclusion proofs"
```

---

## Phase 4 — Soroban attestation contract

**Why / gate:** The contract is the differentiator: it does not trust the submitted `L`; it verifies the receipt and binds it to the issuer's **live** USDC SAC balance. Built test-first against a mock verifier and a test SAC so behavior is locked before touching testnet.

### Task 4.1: Contract crate + constructor + storage

**Files:**
- Create: `contracts/attestation/Cargo.toml`, `contracts/attestation/src/lib.rs`

**Interfaces:**
- Produces: `Config`, `Attestation`, `DataKey`, `Error`; `ProofOfReserves::__constructor(admin, image_id, reserve, usdc_sac, verifier_router)`; `config() -> Option<Config>`.

- [ ] **Step 1: Contract `Cargo.toml` (own workspace, edition 2024, soroban-sdk 25.1.0)**

```toml
[package]
name = "attestation"
version = "0.1.0"
edition = "2024"

[workspace]

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
soroban-sdk = "25.1.0"
risc0-interface = { git = "https://github.com/NethermindEth/stellar-risc0-verifier", package = "risc0-interface" }

[dev-dependencies]
soroban-sdk = { version = "25.1.0", features = ["testutils"] }

[profile.release]
opt-level = "z"
overflow-checks = true
debug = 0
strip = "symbols"
panic = "abort"
codegen-units = 1
lto = true
```

- [ ] **Step 2: Constructor + storage + types** (`contracts/attestation/src/lib.rs`)

```rust
#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Bytes,
    BytesN, Env,
};
use risc0_interface::RiscZeroVerifierRouterClient;

#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub admin: Address,
    pub image_id: BytesN<32>,
    pub reserve: Address,
    pub usdc_sac: Address,
    pub verifier_router: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct Attestation {
    pub root: BytesN<32>,
    pub liabilities: i128,
    pub reserves: i128,
    pub snapshot: u64,
    pub count: u32,
    pub solvent: bool,
}

#[contracttype]
pub enum DataKey {
    Config,
    Attestation(u64),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    Insolvent = 2,
}

#[contract]
pub struct ProofOfReserves;

#[contractimpl]
impl ProofOfReserves {
    pub fn __constructor(
        env: Env,
        admin: Address,
        image_id: BytesN<32>,
        reserve: Address,
        usdc_sac: Address,
        verifier_router: Address,
    ) {
        env.storage().instance().set(
            &DataKey::Config,
            &Config { admin, image_id, reserve, usdc_sac, verifier_router },
        );
    }

    pub fn config(env: Env) -> Option<Config> {
        env.storage().instance().get(&DataKey::Config)
    }
}
```

- [ ] **Step 3: Build to verify it compiles against the real verifier interface**

Run: `cd contracts/attestation && cargo build`
Expected: compiles. (First build fetches the `risc0-interface` git dep — confirms soroban-sdk 25.1.0 compatibility.)

- [ ] **Step 4: Commit**

```bash
git add contracts/attestation/Cargo.toml contracts/attestation/src/lib.rs
git commit -m "feat(contract): attestation contract skeleton, constructor, storage types"
```

### Task 4.2: `submit_proof` — verify receipt, read live reserves, enforce `R ≥ L`, store

**Files:**
- Modify: `contracts/attestation/src/lib.rs`
- Create: `contracts/attestation/src/test.rs`

**Interfaces:**
- Consumes: `RiscZeroVerifierRouterClient::verify(&seal, &image_id, &journal_digest)`; `token::TokenClient::balance`.
- Produces: `submit_proof(seal: Bytes, root: BytesN<32>, total: u64, snapshot: u64, count: u32) -> Result<Attestation, Error>`; `get_attestation(snapshot: u64) -> Option<Attestation>`.

- [ ] **Step 1: Write the failing tests** (`contracts/attestation/src/test.rs`)

```rust
#![cfg(test)]
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contract, contractimpl, symbol_short, token, Address, Bytes, BytesN, Env};
use risc0_interface::VerifierError;
use crate::{Attestation, Error, ProofOfReserves, ProofOfReservesClient};

// --- Mock router: succeeds unless a "reject" flag is set, then it traps. ---
#[contract]
pub struct MockVerifier;

#[contractimpl]
impl MockVerifier {
    pub fn set_reject(env: Env, v: bool) {
        env.storage().instance().set(&symbol_short!("reject"), &v);
    }
    pub fn verify(
        env: Env,
        _seal: Bytes,
        _image_id: BytesN<32>,
        _journal: BytesN<32>,
    ) -> Result<(), VerifierError> {
        let reject: bool = env.storage().instance().get(&symbol_short!("reject")).unwrap_or(false);
        if reject {
            panic!("invalid proof");
        }
        Ok(())
    }
}

struct Fixture {
    env: Env,
    client: ProofOfReservesClient<'static>,
    reserve: Address,
    usdc: Address,
    verifier: Address,
    admin: Address,
}

fn setup(reserve_amount: i128) -> Fixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let reserve = Address::generate(&env);

    // Test SAC + mint reserves to the reserve address.
    let sac_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(sac_admin.clone());
    let usdc = sac.address();
    token::StellarAssetClient::new(&env, &usdc).mint(&reserve, &reserve_amount);

    let verifier = env.register(MockVerifier, ());
    let image_id = BytesN::from_array(&env, &[0u8; 32]);

    let contract_id = env.register(
        ProofOfReserves,
        (admin.clone(), image_id, reserve.clone(), usdc.clone(), verifier.clone()),
    );
    let client = ProofOfReservesClient::new(&env, &contract_id);

    Fixture { env, client, reserve, usdc, verifier, admin }
}

#[test]
fn solvent_attestation_is_stored() {
    let f = setup(500_000_000); // reserves
    let root = BytesN::from_array(&f.env, &[9u8; 32]);
    let seal = Bytes::from_array(&f.env, &[1, 2, 3, 4]);

    let att = f.client.submit_proof(&seal, &root, &400_000_000u64, &1_700_000_000u64, &7u32);
    assert!(att.solvent);
    assert_eq!(att.liabilities, 400_000_000);
    assert_eq!(att.reserves, 500_000_000);

    let stored = f.client.get_attestation(&1_700_000_000u64).unwrap();
    assert_eq!(stored.root, root);
    assert_eq!(stored.solvent, true);
}

#[test]
fn insolvent_is_rejected() {
    let f = setup(100_000_000); // reserves < liabilities
    let root = BytesN::from_array(&f.env, &[9u8; 32]);
    let seal = Bytes::from_array(&f.env, &[1, 2, 3, 4]);

    let res = f.client.try_submit_proof(&seal, &root, &400_000_000u64, &1u64, &7u32);
    assert_eq!(res, Err(Ok(Error::Insolvent)));
    assert!(f.client.get_attestation(&1u64).is_none());
}

#[test]
fn invalid_proof_traps() {
    let f = setup(500_000_000);
    // Flip the mock verifier to reject.
    let mock = crate::test::MockVerifierClient::new(&f.env, &f.verifier);
    mock.set_reject(&true);

    let root = BytesN::from_array(&f.env, &[9u8; 32]);
    let seal = Bytes::from_array(&f.env, &[1, 2, 3, 4]);
    let res = f.client.try_submit_proof(&seal, &root, &10u64, &2u64, &7u32);
    assert!(res.is_err()); // verification trap surfaces as an error
}
```
Add to `lib.rs`: `#[cfg(test)] mod test;`

- [ ] **Step 2: Run to verify they fail**

Run: `cd contracts/attestation && cargo test`
Expected: FAIL — `submit_proof`/`get_attestation` not found.

- [ ] **Step 3: Implement `submit_proof` + `get_attestation`** (add to the `#[contractimpl]`)

```rust
    pub fn submit_proof(
        env: Env,
        seal: Bytes,
        root: BytesN<32>,
        total: u64,
        snapshot: u64,
        count: u32,
    ) -> Result<Attestation, Error> {
        let config: Config =
            env.storage().instance().get(&DataKey::Config).ok_or(Error::NotInitialized)?;
        config.admin.require_auth();

        // Reconstruct the 52-byte journal EXACTLY as the guest committed it:
        // root(32) || total:u64 LE || snapshot:u64 LE || count:u32 LE.
        let mut journal = Bytes::new(&env);
        journal.extend_from_array(&root.to_array());
        journal.extend_from_array(&total.to_le_bytes());
        journal.extend_from_array(&snapshot.to_le_bytes());
        journal.extend_from_array(&count.to_le_bytes());
        let journal_digest = env.crypto().sha256(&journal).to_bytes();

        // Verify the Groth16 receipt through the router (traps if invalid).
        let router = RiscZeroVerifierRouterClient::new(&env, &config.verifier_router);
        router.verify(&seal, &config.image_id, &journal_digest);

        // Bind to LIVE on-chain reserves and enforce solvency.
        let reserves = token::TokenClient::new(&env, &config.usdc_sac).balance(&config.reserve);
        let liabilities = total as i128;
        if reserves < liabilities {
            return Err(Error::Insolvent);
        }

        let attestation = Attestation {
            root: root.clone(),
            liabilities,
            reserves,
            snapshot,
            count,
            solvent: true,
        };
        env.storage().persistent().set(&DataKey::Attestation(snapshot), &attestation);
        env.events().publish(
            (symbol_short!("attest"), snapshot),
            (root, liabilities, reserves),
        );
        Ok(attestation)
    }

    pub fn get_attestation(env: Env, snapshot: u64) -> Option<Attestation> {
        env.storage().persistent().get(&DataKey::Attestation(snapshot))
    }
```

- [ ] **Step 4: Run to verify they pass**

Run: `cd contracts/attestation && cargo test`
Expected: PASS — `solvent_attestation_is_stored`, `insolvent_is_rejected`, `invalid_proof_traps`.

- [ ] **Step 5: Build the Wasm artifact**

Run: `cd contracts/attestation && stellar contract build`
Expected: produces `target/wasm32v1-none/release/attestation.wasm`.

- [ ] **Step 6: Commit**

```bash
git add contracts/attestation/src
git commit -m "feat(contract): submit_proof verifies receipt, reads live USDC reserves, enforces R>=L"
```

---

## Phase 5 — Wire host → contract on testnet

**Why / gate:** End-to-end on testnet: deploy our contract pointed at the testnet router + a real USDC-style SAC + a funded reserve, then submit the real `proof.json` and read back the attestation. This is the path the demo video records.

### Task 5.1: Deploy our contract and submit the real proof

**Files:**
- Create: `scripts/demo.sh` (orchestration, English comments)

**Interfaces:**
- Consumes: `out/proof.json` (Task 3.1), `ROUTER_TESTNET` (Task 0.4).

- [ ] **Step 1: Create a controllable demo asset + reserve, mint reserves**

For the demo, issue a controllable test asset (so we can set reserves precisely). `scripts/demo.sh` (run top-to-bottom; English comments):
```bash
#!/usr/bin/env bash
set -euo pipefail
NET=testnet
ISSUER=issuer            # stellar key (admin + asset issuer)
RESERVE_KEY=reserve

stellar keys generate "$RESERVE_KEY" --network "$NET" || true
stellar keys fund "$RESERVE_KEY" --network "$NET" || true
ISSUER_ADDR=$(stellar keys address "$ISSUER")
RESERVE_ADDR=$(stellar keys address "$RESERVE_KEY")

# Wrap a classic asset "USDC:<issuer>" into a SAC we control.
USDC_SAC=$(stellar contract id asset --asset "USDC:${ISSUER_ADDR}" --network "$NET")
stellar contract asset deploy --asset "USDC:${ISSUER_ADDR}" --source "$ISSUER" --network "$NET" || true

# Reserve must trust + hold the asset. Mint slightly ABOVE total liabilities
# (out/proof.json total = 4_471_750_000 stroops). Mint 500 USDC = 5_000_000_000.
stellar tx new change-trust  --source "$RESERVE_KEY" --network "$NET" --line "USDC:${ISSUER_ADDR}" --limit 100000000000
stellar tx new payment       --source "$ISSUER"     --network "$NET" --destination "$RESERVE_ADDR" --asset "USDC:${ISSUER_ADDR}" --amount 5000000000

echo "USDC_SAC=$USDC_SAC"
echo "RESERVE_ADDR=$RESERVE_ADDR"
echo "ISSUER_ADDR=$ISSUER_ADDR"
```
Run: `bash scripts/demo.sh`
Expected: prints `USDC_SAC`, `RESERVE_ADDR`, `ISSUER_ADDR`. Save them.

- [ ] **Step 2: Deploy our attestation contract with constructor args**

```bash
IMAGE_ID=$(python3 -c "import json;print(json.load(open('out/proof.json'))['image_id_hex'])")
CONTRACT_ID=$(stellar contract deploy \
  --wasm contracts/attestation/target/wasm32v1-none/release/attestation.wasm \
  --source issuer --network testnet \
  -- \
  --admin "$ISSUER_ADDR" \
  --image_id "$IMAGE_ID" \
  --reserve "$RESERVE_ADDR" \
  --usdc_sac "$USDC_SAC" \
  --verifier_router "$ROUTER_TESTNET")
echo "CONTRACT_ID=$CONTRACT_ID"
```
Expected: prints a contract id. (Constructor args are passed after the `--`.)

- [ ] **Step 3: Submit the real proof**

```bash
SEAL=$(python3 -c "import json;print(json.load(open('out/proof.json'))['seal_hex'])")
ROOT=$(python3 -c "import json;print(json.load(open('out/proof.json'))['root_hex'])")
TOTAL=$(python3 -c "import json;print(json.load(open('out/proof.json'))['total'])")
SNAP=$(python3 -c "import json;print(json.load(open('out/proof.json'))['snapshot'])")
COUNT=$(python3 -c "import json;print(json.load(open('out/proof.json'))['count'])")

stellar contract invoke --source issuer --network testnet --id "$CONTRACT_ID" \
  -- submit_proof \
  --seal "$SEAL" --root "$ROOT" --total "$TOTAL" --snapshot "$SNAP" --count "$COUNT"
```
Expected: returns an `Attestation` with `solvent: true`, `reserves: 5000000000`, `liabilities: 4471750000`.

- [ ] **Step 4: Read it back**

```bash
stellar contract invoke --send=no --source issuer --network testnet --id "$CONTRACT_ID" \
  -- get_attestation --snapshot "$SNAP"
```
Expected: the stored attestation JSON. **This is the on-chain proof-of-reserves the demo shows.**

- [ ] **Step 5: Negative check (insolvency)** — submit a `total` above reserves and confirm it reverts.

```bash
stellar contract invoke --source issuer --network testnet --id "$CONTRACT_ID" \
  -- submit_proof --seal "$SEAL" --root "$ROOT" --total 99999999999 --snapshot 2 --count "$COUNT" || echo "correctly rejected"
```
Expected: error / `correctly rejected` (note: this will fail at the verifier step too since the digest changes — both are valid rejections; the solvency path is unit-tested in Task 4.2).

- [ ] **Step 6: Commit**

```bash
git add scripts/demo.sh
git commit -m "feat(demo): testnet orchestration — deploy contract, submit real proof, read attestation"
```

---

## Phase 6 — Off-chain user inclusion verifier (CLI)

**Why / gate:** Each user must independently confirm their balance was counted in `M`. Pure Merkle verification reusing `por-core` — no ZK, no chain calls beyond reading the published root.

### Task 6.1: `verify-inclusion` CLI

**Files:**
- Create: `verifier-cli/Cargo.toml`, `verifier-cli/src/main.rs`

**Interfaces:**
- Consumes: `por-core::{InclusionProof, verify_inclusion}`; `out/inclusion/<id>.json`; `root_hex` from `out/proof.json`.

- [ ] **Step 1: `verifier-cli/Cargo.toml`**

```toml
[package]
name = "verifier-cli"
version = "0.1.0"
edition = "2021"

[dependencies]
por-core = { path = "../por-core" }
serde_json = { workspace = true }
hex = { workspace = true }
anyhow = { workspace = true }
clap = { workspace = true }
```

- [ ] **Step 2: Write the failing integration test** (`verifier-cli/tests/cli.rs`)

```rust
use std::process::Command;

#[test]
fn verifies_a_known_good_proof() {
    // Arrange: build the tree, write a proof + root to temp files via por-core directly.
    use por_core::{MerkleSumTree, Account};
    let accounts = vec![Account { id: 7, balance: 5 }, Account { id: 8, balance: 9 }];
    let tree = MerkleSumTree::build(&accounts).unwrap();
    let proof = tree.inclusion_proof(0).unwrap();
    let dir = std::env::temp_dir();
    let pf = dir.join("por_proof_test.json");
    std::fs::write(&pf, serde_json::to_vec(&proof).unwrap()).unwrap();
    let root_hex = hex::encode(tree.root());

    // Act: run the CLI.
    let out = Command::new(env!("CARGO_BIN_EXE_verifier-cli"))
        .args(["--proof", pf.to_str().unwrap(), "--root", &root_hex])
        .output()
        .unwrap();

    // Assert.
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("INCLUDED"));
}
```
Add `[dev-dependencies] por-core = { path = "../por-core" }`, `serde_json`, `hex` to `verifier-cli/Cargo.toml`.

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p verifier-cli`
Expected: FAIL — binary not implemented.

- [ ] **Step 4: Implement the CLI** (`verifier-cli/src/main.rs`)

```rust
use anyhow::{anyhow, Result};
use clap::Parser;
use por_core::{verify_inclusion, InclusionProof};
use std::{fs, path::PathBuf, process::exit};

#[derive(Parser)]
struct Args {
    /// Path to an inclusion proof JSON (out/inclusion/<id>.json).
    #[arg(long)]
    proof: PathBuf,
    /// The published Merkle root, hex (out/proof.json root_hex / on-chain attestation root).
    #[arg(long)]
    root: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let proof: InclusionProof = serde_json::from_slice(&fs::read(&args.proof)?)?;
    let root_bytes = hex::decode(args.root.trim_start_matches("0x"))?;
    let root: [u8; 32] = root_bytes.try_into().map_err(|_| anyhow!("root must be 32 bytes"))?;

    if verify_inclusion(&proof, &root) {
        println!("INCLUDED  id={} balance={} stroops", proof.id, proof.balance);
        Ok(())
    } else {
        println!("NOT INCLUDED  id={} (proof does not match root)", proof.id);
        exit(1);
    }
}
```

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p verifier-cli`
Expected: PASS. Also manual: `cargo run -p verifier-cli -- --proof out/inclusion/1001.json --root $(python3 -c "import json;print(json.load(open('out/proof.json'))['root_hex'])")` prints `INCLUDED id=1001 ...`.

- [ ] **Step 6: Commit**

```bash
git add verifier-cli
git commit -m "feat(verifier-cli): off-chain Merkle inclusion self-verification"
```

---

## Phase 7 — Dashboard, README, demo

**Why / gate:** Make the work demoable and judgeable. Functionality first; visual polish is deferred to `open-design` per the design spec. The README must honestly document the known limitations.

### Task 7.1: Functional demo dashboard (read attestation; verify inclusion)

**Files:**
- Create: `frontend/index.html`, `frontend/app.js`, `frontend/config.json`

**Interfaces:**
- Consumes: `CONTRACT_ID` (Task 5.1), Stellar testnet RPC; `out/inclusion/<id>.json` files.

- [ ] **Step 1: Config**

`frontend/config.json`:
```json
{
  "rpcUrl": "https://soroban-testnet.stellar.org",
  "networkPassphrase": "Test SDF Network ; September 2015",
  "contractId": "PUT_CONTRACT_ID_HERE",
  "snapshot": 1700000000
}
```
Replace `PUT_CONTRACT_ID_HERE` with the `CONTRACT_ID` printed in Task 5.1 Step 2, and `snapshot` with the value used when submitting the proof.

- [ ] **Step 2: Public view — read + display the on-chain attestation**

`frontend/index.html` (minimal, functional; styling later via open-design):
```html
<!doctype html>
<html>
<head><meta charset="utf-8"><title>ZK Proof-of-Reserves</title></head>
<body>
  <h1>ZK Proof-of-Reserves (Stellar)</h1>
  <section id="public">
    <h2>Solvency attestation</h2>
    <div id="status">loading…</div>
    <dl>
      <dt>Reserves R</dt><dd id="reserves">—</dd>
      <dt>Liabilities L</dt><dd id="liabilities">—</dd>
      <dt>Users N</dt><dd id="count">—</dd>
      <dt>Merkle root M</dt><dd id="root" style="word-break:break-all">—</dd>
    </dl>
  </section>
  <section id="user">
    <h2>Verify your inclusion</h2>
    <textarea id="proofJson" rows="8" cols="80" placeholder="paste your inclusion proof JSON"></textarea><br>
    <button id="verifyBtn">Verify against on-chain root</button>
    <div id="inclusionResult"></div>
  </section>
  <script type="module" src="app.js"></script>
</body>
</html>
```

- [ ] **Step 3: `app.js` — contract read via stellar-sdk + inclusion check**

`frontend/app.js`:
```js
import {
  Contract, SorobanRpc, TransactionBuilder, Account, scValToNative, nativeToScVal,
} from "https://esm.sh/@stellar/stellar-sdk@13";

const cfg = await (await fetch("./config.json")).json();
const server = new SorobanRpc.Server(cfg.rpcUrl);

async function loadAttestation() {
  const contract = new Contract(cfg.contractId);
  // Build a read-only simulated invocation of get_attestation(snapshot).
  const dummy = new Account("GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF5", "0");
  const tx = new TransactionBuilder(dummy, { fee: "100", networkPassphrase: cfg.networkPassphrase })
    .addOperation(contract.call("get_attestation", nativeToScVal(cfg.snapshot, { type: "u64" })))
    .setTimeout(30).build();
  const sim = await server.simulateTransaction(tx);
  const att = scValToNative(sim.result.retval);
  if (!att) { document.getElementById("status").textContent = "No attestation yet."; return; }
  const usdc = (n) => (Number(n) / 1e7).toLocaleString() + " USDC";
  document.getElementById("status").textContent = att.solvent ? "✅ FULLY RESERVED" : "⚠️ UNDER-RESERVED";
  document.getElementById("reserves").textContent = usdc(att.reserves);
  document.getElementById("liabilities").textContent = usdc(att.liabilities);
  document.getElementById("count").textContent = att.count;
  document.getElementById("root").textContent = toHex(att.root);
  window.__root = toHex(att.root);
}

function toHex(bytes) { return [...bytes].map(b => b.toString(16).padStart(2, "0")).join(""); }

document.getElementById("verifyBtn").onclick = async () => {
  // Inclusion check runs in the browser via the por-core wasm module (Step 4).
  const proof = JSON.parse(document.getElementById("proofJson").value);
  const ok = await window.verifyInclusionWasm(proof, window.__root);
  document.getElementById("inclusionResult").textContent =
    ok ? `✅ INCLUDED — id ${proof.id}, balance ${proof.balance} stroops`
       : "❌ NOT INCLUDED — does not match the on-chain root";
};

loadAttestation().catch(e => document.getElementById("status").textContent = "error: " + e.message);
```
> Note: the exact `stellar-sdk` read-call ergonomics (building the `u64` ScVal, simulating) should be validated against the installed SDK version during this task; adjust `require_u64`/imports to the SDK's current API if needed. The functional goal is: simulate `get_attestation(snapshot)` and render the returned struct.

- [ ] **Step 4: Browser inclusion verification via `por-core` Wasm** (reuses the exact prover hashing)

Create `por-wasm/Cargo.toml`:
```toml
[package]
name = "por-wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
por-core = { path = "../por-core" }
wasm-bindgen = "0.2"
serde-wasm-bindgen = "0.6"
```
Create `por-wasm/src/lib.rs`:
```rust
use por_core::{verify_inclusion, InclusionProof};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn verify_inclusion_hex(proof_json: &str, root_hex: &str) -> Result<bool, JsValue> {
    let proof: InclusionProof =
        serde_json::from_str(proof_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let root_bytes = hex::decode(root_hex.trim_start_matches("0x"))
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let root: [u8; 32] = root_bytes.try_into().map_err(|_| JsValue::from_str("root must be 32 bytes"))?;
    Ok(verify_inclusion(&proof, &root))
}
```
Add `serde_json` and `hex` to `por-wasm` deps. Build with `wasm-pack build por-wasm --target web --out-dir ../frontend/pkg`, and in `app.js` import the generated module to define `window.verifyInclusionWasm` calling `verify_inclusion_hex(JSON.stringify(proof), root)`.
> This crate is its own cdylib (not in the host workspace) to keep the wasm32-unknown-unknown build isolated. If `wasm-pack` setup proves time-consuming, the fallback is to call `verifier-cli` (Phase 6) for inclusion and have the dashboard show only the public attestation — note this honestly in the README.

- [ ] **Step 5: Serve and manually verify in a browser**

Run: `cd frontend && python3 -m http.server 8080`
Open `http://localhost:8080`, confirm: status shows ✅ FULLY RESERVED with R/L/N/root from testnet; pasting `out/inclusion/1001.json` shows ✅ INCLUDED; editing the balance shows ❌ NOT INCLUDED. (UI is unstyled — visuals come later via open-design.)

- [ ] **Step 6: Commit**

```bash
git add frontend por-wasm
git commit -m "feat(frontend): functional PoR dashboard — live attestation + browser inclusion check"
```

### Task 7.2: README (honest, judge-facing)

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write `README.md`** covering, in order:
  1. **The problem** — post-FTX: prove an entity's USDC reserves cover all user liabilities without exposing individual balances.
  2. **Where ZK is load-bearing** — proving `R ≥ Σ balanceᵢ` + every balance valid while keeping balances private; without ZK you must either expose balances or be trusted.
  3. **The Stellar integration** — the Soroban contract verifies the RISC Zero Groth16 receipt **and reads the issuer's live USDC SAC balance** to bind the proof to real on-chain funds (the differentiator); deployed NethermindEth router.
  4. **Architecture diagram** (text) + **how to run** (Phase 0 install → `cargo run -p host` → `scripts/demo.sh` → dashboard).
  5. **Known limitations (verbatim honesty):** liability completeness (PoR proves `R ≥ sum of *included* liabilities`; mitigated by user inclusion self-checks), temporal gap (`L` at `T`, `R` at `T'`), mock dataset stands in for a real internal ledger, the NethermindEth verifier is unaudited and self-deployed.
  6. **What's mock vs real:** balances are a mock dataset; reserves are read live from a real (testnet) SAC; the proof and on-chain verification are real.

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: judge-facing README with problem, ZK rationale, Stellar integration, honest limitations"
```

### Task 7.3: Demo video checklist

**Files:**
- Create: `docs/demo-script.md`

- [ ] **Step 1: Write a 2–3 min shot list:** (1) state the problem in one line; (2) show `cargo run -p host` producing a real Groth16 proof; (3) `scripts/demo.sh` submitting on testnet → contract verifies receipt + reads live USDC reserves → `solvent: true`; (4) dashboard shows ✅ FULLY RESERVED with R/L/N; (5) a user pastes their inclusion proof → ✅ INCLUDED; (6) tamper the balance → ❌ NOT INCLUDED; (7) one sentence on why ZK is essential. Keep under 3 minutes.

- [ ] **Step 2: Commit**

```bash
git add docs/demo-script.md
git commit -m "docs: demo video shot list"
```

---

## Final verification checklist (run after Phase 7)

- [ ] `cargo test -p por-core` green (tree, inclusion, journal).
- [ ] `cargo test -p host --test guest_journal` green (guest I/O).
- [ ] `cd contracts/attestation && cargo test` green (solvent / insolvent / invalid-proof).
- [ ] `cargo test -p verifier-cli` green.
- [ ] Real Groth16 `out/proof.json` produced; `submit_proof` on testnet returns `solvent: true`; `get_attestation` reads it back.
- [ ] Dashboard renders the live attestation and verifies/【rejects】inclusion proofs.
- [ ] README documents every limitation in the design spec §9.
- [ ] No secrets committed (`git log --stat | grep -Ei 'identity|secret|\.key|\.pem'` returns nothing).
