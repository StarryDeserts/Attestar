# Stellar ZK Proof-of-Reserves — Design Spec

- **Date:** 2026-06-26
- **Hackathon:** Stellar Hacks — Real-World ZK
- **Status:** Approved design, ready for implementation planning

## 1. Problem

Any entity that custodies stablecoins for users — a stablecoin issuer, a
custodian, an on-chain neobank, an exchange — faces a trust problem made
painfully concrete by the FTX collapse: how do users and the public know the
entity actually holds enough reserves to cover everyone's balances?

- Publishing every account balance is a privacy and competitive non-starter.
- Asking the public to "trust us" is exactly what failed.

This project lets such an entity **prove on Stellar that its real USDC reserves
fully cover the sum of all user liabilities, without revealing any individual
balance**, and lets each user independently verify that their balance was counted.

## 2. Core claim

At snapshot time `T`:

- `L = Σ balanceᵢ` — the sum of all user liabilities
- every `balanceᵢ` satisfies `0 ≤ balanceᵢ < 2^64` (no negative or overflowing
  balances used to fake solvency)
- the balance set is committed under Merkle-sum-tree root `M`
- and, checked **live on-chain**, the issuer's real USDC reserve `R ≥ L`

Individual `balanceᵢ` stay private. `R`, `L`, `M`, `T`, `N` are public.

## 3. Why ZK is load-bearing

The whole value is proving an aggregate property (`R ≥ Σ balanceᵢ`, all balances
valid) while keeping the inputs (individual balances) secret. Without ZK you must
either expose all balances (privacy violation) or be trusted (the thing that
failed). ZK is not decoration here — it is the only thing that makes
*private + verifiable* solvency possible.

## 4. Why Stellar / why these choices

- Stellar is real-world money rails (USDC, RWAs, custodians). Proof-of-reserves
  is a real pain point for exactly the entities already on Stellar.
- The Soroban verifier contract reads the issuer's **actual live USDC balance**
  via the asset's SAC (Stellar Asset Contract), binding the ZK proof to real
  on-chain funds rather than a self-asserted number. **This is the project's
  differentiator** — the contract is not a dumb proof-checker; it ties the proof
  to live ledger state.
- Protocol 25/26 BN254 host functions make RISC Zero proof verification on
  Stellar affordable.

## 5. ZK stack: RISC Zero

Chosen because the prover logic — build a Merkle-sum tree over a variable number
`N` of accounts, range-check each balance, sum them — is ~50 lines of ordinary
Rust in a zkVM guest, versus a fixed-`N`, constraint-heavy Circom circuit. It
fits a Rust-strong solo author and supports variable `N`. Verified on-chain via
the existing [stellar-risc0-verifier](https://github.com/NethermindEth/stellar-risc0-verifier)
(Nethermind).

Alternative considered: Circom/Groth16 — cheaper verification, smaller proofs,
but fixed `N` and far more circuit-authoring effort. Rejected for the solo time
budget.

## 6. Components

### 6.1 RISC Zero guest (Rust zkVM)
- Private input: `Vec<(user_id, balance: u64)>`
- Asserts `balance < 2^64` (type-enforced) and accumulates `L` with overflow checks
- Builds Merkle-sum tree:
  - leaf = `H(user_id || balance)` carrying `balance`
  - node = `H(left.hash || right.hash || left.sum || right.sum)` carrying
    `left.sum + right.sum`
- Public journal output: `(root M, total L, snapshot T, count N)`
- Hash function: default SHA-256 for guest simplicity; revisit Poseidon (native
  on Stellar P25) if on-chain inclusion verification needs a ZK-friendly hash.
  Decide in the implementation plan.

### 6.2 Soroban verifier / attestation contract (Rust)
- `init(image_id, reserve_address, usdc_sac_address, admin)`
- `submit_proof(receipt, journal = (M, L, T, N))`:
  1. verify `receipt` against the pinned `image_id` via stellar-risc0-verifier
  2. read live reserves: `R = usdc_sac.balance(reserve_address)`
  3. require `R ≥ L`
  4. store `Attestation { M, L, R, T, status: SOLVENT }`, emit an event
- `get_attestation(snapshot_id) -> Attestation` (view)
- optional `verify_inclusion(leaf, path, snapshot_id) -> bool` (view)

### 6.3 User inclusion verifier (off-chain CLI, later web)
- Input: a user's `(user_id, balance, siblings[])` plus on-chain `M`
- Recompute the leaf, walk the path, compare to `M`, confirm the running sums are
  consistent → "your balance is included ✓"
- Pure Merkle verification; no ZK needed.

### 6.4 Prover / host CLI (Rust)
- Load (mock) balance dataset → run the RISC Zero prover → submit the receipt to
  the Soroban contract via the Stellar SDK → emit per-user inclusion proofs.

### 6.5 Demo dashboard (frontend) — in scope
- Public view: `Reserves R / Liabilities L / Status ✅ FULLY RESERVED (proven at T)`,
  read from the contract.
- User view: paste an inclusion proof → "✅ your balance is included".
- Visual design to be produced later via open-design; build the functional
  contract + flows first.

## 7. Data flow (end-to-end)

1. Issuer exports user balances (mock dataset for the demo) at snapshot `T`.
2. Host runs the guest → builds the sum-tree, computes `(M, L)`, produces a receipt.
3. Host calls `submit_proof(receipt, (M, L, T, N))`.
4. Contract verifies the receipt → reads live `R` via the USDC SAC → asserts
   `R ≥ L` → stores the attestation and emits an event.
5. The public reads the attestation: "provably fully reserved".
6. Each user verifies their Merkle inclusion against the published `M`.

## 8. Privacy boundary

- **Hidden:** every individual `balanceᵢ` and identity detail.
- **Public:** `R`, `L`, `M`, `T`, `N`, solvency status.
- **Stretch:** hide `L` too — prove `R ≥ L` without revealing `L`.

## 9. Known limitations (document honestly in README)

- **Liability completeness:** like all PoR, this proves `R ≥ sum of *included*
  liabilities`; it cannot force the issuer to include every user. Mitigated by
  user inclusion self-checks (enough users checking makes omissions detectable).
  This is the well-known caveat of every real PoR system (Binance, Kraken, …).
- **Temporal gap:** `L` is proven at snapshot `T`; `R` is read at verification
  time `T'`. Small window, documented.
- **Mock dataset** stands in for the issuer's real internal ledger.

## 10. Build sequence

1. Guest: sum-tree + range checks + journal, with Rust unit tests.
2. Host CLI: prove + generate inclusion paths.
3. Soroban contract: receipt verification + SAC reserve read + `R ≥ L` + store,
   tested on testnet.
4. Wire host → contract submission.
5. Inclusion verifier (CLI → web).
6. Dashboard + README + demo video.

## 11. Out of scope (YAGNI)

- Real internal-ledger integration (mock for the demo).
- Hiding `L` (stretch only).
- Recursive / aggregated proofs.
- Multi-asset reserves (USDC only for the demo).
