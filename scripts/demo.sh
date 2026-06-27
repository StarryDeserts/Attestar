#!/usr/bin/env bash
#
# Submit the ZK Proof-of-Reserves to the live Soroban attestation contract and
# print the resulting on-chain solvency attestation.
#
# What the contract does when you call submit_proof:
#   1. verifies the RISC Zero Groth16 receipt via the NethermindEth verifier router
#   2. reads the issuer's LIVE USDC reserve from the Stellar Asset Contract (SAC)
#   3. stores the attestation only if reserves >= liabilities
#
# Requirements: stellar-cli, python3, and (for submit) the contract `admin`
# identity in your local stellar config. Reads contract IDs from deployment.json
# and proof fields from out/proof.json.
#
# Usage:
#   ./scripts/demo.sh              # submit the proof, then show the attestation
#   ./scripts/demo.sh --show-only  # only read the current on-chain attestation
#
# Env overrides: NETWORK, SOURCE (admin identity name), CONTRACT_ID, PROOF
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$ROOT_DIR"

NETWORK="${NETWORK:-testnet}"
SOURCE="${SOURCE:-deployer}"
PROOF="${PROOF:-out/proof.json}"

j() { python3 -c "import json,sys; print(json.load(open(sys.argv[1]))$2)" "$1"; }

CONTRACT_ID="${CONTRACT_ID:-$(j deployment.json "['contracts']['attestation']")}"
SNAPSHOT="$(j deployment.json "['snapshot']")"

show_attestation() {
  echo ""
  echo "On-chain attestation (snapshot=$SNAPSHOT):"
  stellar contract invoke --id "$CONTRACT_ID" --source "$SOURCE" --network "$NETWORK" -- \
    get_attestation --snapshot "$SNAPSHOT"
}

if [[ "${1:-}" == "--show-only" ]]; then
  show_attestation
  exit 0
fi

if [[ ! -f "$PROOF" ]]; then
  echo "error: $PROOF not found — generate it first:" >&2
  echo "  cargo run -p host -- --balances data/mock-balances.json --snapshot $SNAPSHOT" >&2
  exit 1
fi

SEAL="$(j "$PROOF" "['seal_hex']")"
ROOT="$(j "$PROOF" "['root_hex']")"
TOTAL="$(j "$PROOF" "['total']")"
PSNAP="$(j "$PROOF" "['snapshot']")"
COUNT="$(j "$PROOF" "['count']")"

echo "Contract : $CONTRACT_ID  ($NETWORK)"
echo "Proof    : root=$ROOT"
echo "           total=$TOTAL stroops  snapshot=$PSNAP  count=$COUNT"
echo ""
echo "Submitting proof — contract will verify the receipt, read live USDC reserves, enforce R >= L ..."
stellar contract invoke --id "$CONTRACT_ID" --source "$SOURCE" --network "$NETWORK" --send=yes -- \
  submit_proof --seal "$SEAL" --root "$ROOT" --total "$TOTAL" --snapshot "$PSNAP" --count "$COUNT"

show_attestation
