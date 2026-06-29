# Attestar — ZK Proof-of-Reserves on Stellar

> Prove an exchange's USDC reserves cover **every** user's balance — verified on-chain by a Soroban contract — **without revealing any individual balance.**

**Attestar** — built for **Stellar Hacks: Real-World ZK**.

A custodian commits to its per-user liabilities under a Merkle-sum-tree root `M`, generates a RISC Zero zkVM proof that `total = Σ balanceᵢ` (with every `0 ≤ balanceᵢ < 2⁶⁴`), wraps it as a Groth16 receipt, and submits it to a Soroban contract. The contract verifies the receipt **and reads the issuer's live on-chain USDC balance**, storing an attestation only if `reserves ≥ liabilities`. Any user can then independently check that their balance was included in `M`.

## Try it

- **🟢 Live dashboard — <https://starrydeserts.github.io/Attestar/>** — reads the attestation live from the Soroban contract and verifies a Merkle inclusion proof entirely in your browser. No install required.
- **▶ Demo video:** _add your YouTube / Loom link here_ — a narrated ~80-second walkthrough.
- **🔗 On-chain:** attestation contract [`CBMZ…ZTBK`](https://stellar.expert/explorer/testnet/contract/CBMZGJJYJCBNEG3HHPEE42XPP6TNINKWK2SM7XM3H7DNXNAPZXI2ZTBK) · example `submit_proof` [tx `719445c8…`](https://stellar.expert/explorer/testnet/tx/719445c8a625ec64e99a67af5b6011c89816f900aa5bb3d2eb0c54cafe7f51a0)

---

## 1. The problem

After FTX, "trust me, the funds are there" is not good enough. Exchanges and custodians need to prove **solvency** — that real reserves `R` cover the sum of all customer liabilities `L` — but the naive ways to do it are both bad:

- **Publish every balance** → leaks every customer's holdings.
- **Trust an auditor's PDF** → point-in-time, off-chain, unverifiable, gameable.

We want a third option: a **public, on-chain, cryptographically verifiable** solvency guarantee that keeps individual balances **private**.

## 2. Where zero-knowledge is load-bearing

The ZK proof attests, in zero knowledge, that:

1. `total = Σ balanceᵢ` over all accounts in the set, and
2. every `balanceᵢ` is a valid non-negative `u64` (no negative balances smuggled in to fake solvency), and
3. all of it is committed under a single public **Merkle-sum-tree root** `M`.

…while revealing **only** the aggregate `total`, the root `M`, the account `count`, and the `snapshot` time — never an individual balance.

Without ZK you are forced back to the two bad options: expose balances, or be trusted. ZK is what makes a *private-yet-verifiable* solvency statement possible. The proof is not decoration — it is the only thing standing between "publicly auditable" and "publicly naked."

The commitment is a **Merkle-sum tree**: each leaf carries `(hash, balance)`, each internal node carries `(hash(children), sum(children))`. The root's `sum` is the proven total, and any user can produce a short inclusion proof showing their `(id, balance)` is part of that sum. Hashing is domain-separated (`0x00` leaf / `0x01` node / `0x02` padding) and the tree is padded to a power of two with zero-sum leaves.

## 3. The Stellar integration (the differentiator)

A self-deployed Soroban contract (`contracts/attestation`) is the verifier and the source of truth:

1. **Verifies the proof.** It reconstructs the exact 52-byte journal the guest committed and calls the deployed [NethermindEth `stellar-risc0-verifier`](https://github.com/NethermindEth/stellar-risc0-verifier) router to verify the Groth16 receipt against the guest image ID. An invalid proof traps — nothing is stored.
2. **Reads live on-chain reserves.** It calls `balance()` on the **real USDC Stellar Asset Contract (SAC)** for the issuer's reserve account. This is the key move: the liabilities come from a *proof*, the reserves come from *live chain state* — the attestation binds the two together.
3. **Enforces solvency.** It stores the attestation only if `reserves ≥ liabilities`; otherwise it returns `Insolvent`.

So the contract doesn't just check a proof in a vacuum — it anchors a zero-knowledge liability proof to **actual money on Stellar**. That binding is the whole point.

### Journal layout (the cross-boundary contract)
The guest commits, and the contract reconstructs, exactly these 52 bytes (little-endian):

```
root (32 bytes) ‖ total : u64 (8) ‖ snapshot : u64 (8) ‖ count : u32 (4)
```

The contract verifies `sha256(journal)` against the receipt, so any disagreement on a single byte fails verification.

## 4. Architecture

```
                        ┌──────────────────────────────────────────┐
   data/mock-balances   │  por-core  (shared, no_std-friendly)      │
   (per-user ledger) ─► │  Merkle-sum tree · inclusion proofs ·     │
                        │  journal encode/decode                    │
                        └───────────────┬───────────────┬──────────┘
                                        │               │
                  ┌─────────────────────▼──┐         ┌──▼───────────────────┐
                  │  methods/guest (zkVM)   │         │  verifier-cli        │
                  │  build tree, commit     │         │  off-chain inclusion │
                  │  journal                │         │  self-check (users)  │
                  └───────────┬─────────────┘         └──────────────────────┘
                              │ proven by
                  ┌───────────▼─────────────┐
                  │  host (prover CLI)       │  ──►  out/proof.json
                  │  RISC Zero → Groth16     │       out/inclusion/<id>.json
                  └───────────┬─────────────┘
                              │ seal + root + total + snapshot + count
                  ┌───────────▼──────────────────────────────────────────┐
                  │  contracts/attestation  (Soroban)                     │
                  │   1. verify receipt via NethermindEth router          │
                  │   2. read LIVE USDC SAC balance of reserve account    │
                  │   3. require reserves ≥ liabilities → store attest    │
                  └───────────┬──────────────────────────────────────────┘
                              │ get_attestation(snapshot)
                  ┌───────────▼─────────────┐
                  │  Attestar dashboard      │  live attestation + in-browser
                  │  (visuals: open-design)  │  inclusion verification
                  └──────────────────────────┘
```

**Components**
| Crate / dir | Role |
|---|---|
| `por-core` | Merkle-sum tree, inclusion proofs, the 52-byte journal codec. Shared by guest, host, contract, and CLI so the hashing is identical everywhere. |
| `methods/guest` | The RISC Zero guest: reads accounts + snapshot, builds the tree, commits the journal. |
| `host` | Prover CLI: runs the guest, produces a Groth16 receipt, writes `out/proof.json` + per-user inclusion proofs. |
| `contracts/attestation` | Soroban contract: verifies the receipt, reads live USDC reserves, enforces solvency, stores attestations. |
| `verifier-cli` | Pure-Rust off-chain tool for a user to verify their inclusion proof against the published root. |
| `frontend` | **Attestar** dashboard — live attestation + in-browser inclusion verification (visual system from `open-design`). |

## 5. How to run

### Prerequisites
- Rust (stable; this repo builds on 1.94+).
- [RISC Zero toolchain](https://dev.risczero.com/api/zkvm/install): `rzup` with `cargo-risczero` 3.0.5 + `r0vm`.
- For **real Groth16**: Docker + an x86_64 host with ~16 GB RAM, plus `rzup install risc0-groth16`.
- For the contract: the `wasm32v1-none` target and [`stellar-cli`](https://developers.stellar.org/docs/tools/cli/install-cli).

### Test everything
```bash
cargo test                              # por-core (tree/inclusion/journal) + host guest round-trip + verifier-cli
cd contracts/attestation && cargo test  # solvent / insolvent / invalid-proof
```

### Generate a proof
```bash
# Real Groth16 (needs Docker + ~16GB):
cargo run -p host -- --balances data/mock-balances.json --snapshot 1700000000

# Fast wiring check, no Docker (fake receipt):
RISC0_DEV_MODE=1 cargo run -p host -- --balances data/mock-balances.json --snapshot 1700000000
```
Writes `out/proof.json` (seal, image id, root, total, snapshot, count) and `out/inclusion/<id>.json` for every account. The bundled dataset is **7 users totaling 4,466,750,000 stroops (446.675 USDC)**.

### Submit on testnet (against the live deployment)
Everything is already deployed to Stellar **testnet** — contract IDs live in [`deployment.json`](deployment.json). `scripts/demo.sh` reads that manifest plus `out/proof.json`, submits the proof to the live attestation contract (which verifies the receipt, reads live USDC reserves, and enforces `R ≥ L`), and prints the stored attestation:
```bash
./scripts/demo.sh             # submit the proof, then show the attestation  (needs the admin identity)
./scripts/demo.sh --show-only # just read the current on-chain attestation   (no key needed)
```
To redeploy from scratch into your own accounts, see [`scripts/setup-testnet.sh`](scripts/setup-testnet.sh).

**Live testnet deployment**
- **Attestation contract:** [`CBMZGJJYJCBNEG3HHPEE42XPP6TNINKWK2SM7XM3H7DNXNAPZXI2ZTBK`](https://stellar.expert/explorer/testnet/contract/CBMZGJJYJCBNEG3HHPEE42XPP6TNINKWK2SM7XM3H7DNXNAPZXI2ZTBK)
- **Verifier router** (NethermindEth, self-deployed): [`CDIVJXYM53PIG46TDPNOQCXJ7JCAKZB5JLXISS244KXF6LRJCC7PFTFD`](https://stellar.expert/explorer/testnet/contract/CDIVJXYM53PIG46TDPNOQCXJ7JCAKZB5JLXISS244KXF6LRJCC7PFTFD)
- **USDC SAC** (live reserves are read from here): [`CDIEHHQMSJ2EXUWRFXVRJTKCMKGTPADILISCE6UVNR4XAHIRG3LJ6QLD`](https://stellar.expert/explorer/testnet/contract/CDIEHHQMSJ2EXUWRFXVRJTKCMKGTPADILISCE6UVNR4XAHIRG3LJ6QLD)
- **Example `submit_proof` transaction:** [`719445c8…f51a0`](https://stellar.expert/explorer/testnet/tx/719445c8a625ec64e99a67af5b6011c89816f900aa5bb3d2eb0c54cafe7f51a0) — verifies the receipt, reads live USDC reserves, stores `solvent: true`.

### Verify your inclusion (as a user)
```bash
cargo run -p verifier-cli -- \
  --proof out/inclusion/1001.json \
  --root  <root_hex from out/proof.json or the on-chain attestation>
# → INCLUDED  id=1001 balance=125000000 stroops
```

## 6. What's mock vs. real

| | Status |
|---|---|
| Per-user balances (`data/mock-balances.json`) | **Mock** — stands in for a custodian's real internal ledger. |
| The ZK proof (RISC Zero → Groth16) | **Real.** |
| On-chain verification (Soroban contract + verifier router) | **Real.** |
| The reserve balance | **Real, read live** from a USDC SAC on Stellar testnet. |

## 7. Known limitations (honest)

- **Liability completeness.** A proof-of-reserves shows `R ≥ Σ of the *included* liabilities`. A dishonest custodian could omit accounts to shrink `L`. This is mitigated — not eliminated — by **user inclusion self-checks**: every user verifies (via `verifier-cli` or the dashboard) that their balance is in the published root, so dropped liabilities are detectable by the affected users. A complete solution also needs a non-inclusion / total-accounts attestation, which is out of scope here.
- **Temporal gap.** Liabilities are snapshotted at time `T` (off-chain ledger); reserves are read at `T'` (on-chain, at submission). They are close but not perfectly simultaneous.
- **Mock ledger.** The balance set is a fixture, not a feed from a real exchange database.
- **Unaudited verifier.** The NethermindEth `stellar-risc0-verifier` is self-deployed and unaudited; this is a hackathon integration, not a production trust anchor.

## 8. Demo & verify it yourself

**▶ Demo video:** _add your YouTube / Loom link here_ — a narrated ~80-second walkthrough (live attestation → **INCLUDED** → one-digit tamper → **NOT INCLUDED**). Shot list: [`docs/demo-script.md`](docs/demo-script.md).

**Anyone can verify the live attestation in a browser — no install:**

1. Open the **[live dashboard](https://starrydeserts.github.io/Attestar/)**. It reads the attestation straight from the Soroban contract on testnet, so you should see **✅ FULLY RESERVED** — 500 USDC of live reserves against 446.675 USDC of proven liabilities (111.94% coverage), plus the Merkle-sum root.
2. Click **Load sample (1001)**, then **Verify against on-chain root** → **✅ INCLUDED — id 1001**. The proof is folded with por-core's exact hashing in your browser and compared against the live root.
3. Change one digit of the balance and verify again → **❌ NOT INCLUDED** — the recomputed root no longer matches.

**Prefer to run it locally?**

```bash
python3 -m http.server 8765      # from the repo root
# then open http://localhost:8765/frontend/index.html
```

The dashboard is a static page (`frontend/`): it reads the chain through the public testnet RPC and verifies proofs with the same logic as [`verifier-cli`](verifier-cli), shared via [`frontend/por-verify.js`](frontend/por-verify.js) and guarded by a Node self-test.

## License

See repository.
