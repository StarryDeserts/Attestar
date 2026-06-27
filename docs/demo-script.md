# Demo Video Shot List (2–3 min)

Target length: **under 3 minutes**. Record at 1080p, terminal font large enough to read. Keep narration tight — one sentence per beat.

---

## 0. Hook (0:00–0:15)
> "After FTX, 'trust me, the funds are there' isn't good enough. This proves an exchange's USDC reserves cover every user's balance — on Stellar, with zero-knowledge, without revealing anyone's balance."

Show the project title card / repo.

## 1. The proof is real (0:15–0:55)
- Show `data/mock-balances.json` briefly (7 users, balances hidden in the proof).
- Run the prover:
  ```bash
  cargo run -p host -- --balances data/mock-balances.json --snapshot 1700000000
  ```
- Point at the output: `wrote out/proof.json (total=4466750000 stroops, count=7)`.
- Open `out/proof.json` — highlight `seal_hex` (the Groth16 receipt) and `root_hex` (the Merkle-sum root). Say: "individual balances never leave the prover."

## 2. Stellar verifies it on-chain (0:55–1:45)
- Run the submit script:
  ```bash
  ./scripts/demo.sh
  ```
- Narrate what the contract does, in one breath: "The Soroban contract verifies the RISC Zero proof through the deployed verifier router, **then reads the issuer's live USDC balance straight from the on-chain SAC**, and only stores the attestation if reserves ≥ liabilities."
- Show the returned attestation: `solvent: true`, `reserves`, `liabilities`, `count`.
- Show the transaction on the explorer (link in README).

## 3. Anyone can audit (1:45–2:30)
- Open the dashboard (`frontend/`): show **✅ FULLY RESERVED** with reserves / liabilities / user count / root pulled live from testnet.
- A user pastes their inclusion proof (`out/inclusion/1001.json`) → **✅ INCLUDED — id 1001**.
- Tamper one digit of the balance → **❌ NOT INCLUDED**. Say: "the user proves they were counted; the exchange can't quietly drop a liability."

## 4. Why ZK (2:30–2:50)
> "Without ZK you'd have to either publish everyone's balance or just trust the exchange. ZK gives you the third option: a public, verifiable solvency guarantee that keeps every balance private."

End on the repo URL.

---

### Pre-record checklist
- [ ] Real Groth16 proof generated (not dev-mode) — `out/proof.json` has a real `seal_hex`.
- [ ] Contract deployed to testnet; `scripts/demo.sh` points at the live `CONTRACT_ID`.
- [ ] Dashboard `config.json` points at the live contract + snapshot.
- [ ] One solvent run staged; optionally one under-reserved run to show `⚠️ UNDER-RESERVED`.
- [ ] Terminal cleared, font enlarged, secrets not on screen.
