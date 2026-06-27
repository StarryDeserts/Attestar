#!/usr/bin/env bash
#
# Reproducible Stellar testnet bootstrap for the ZK Proof-of-Reserves demo.
# This mirrors the exact steps used to produce the live deployment recorded in
# deployment.json. It creates fresh, friendbot-funded accounts, so running it
# deploys YOUR OWN instance (it will overwrite deployment.json).
#
# Prerequisite — the RISC Zero verifier router must already be deployed (it lives
# in a separate repo). Deploy it once with NethermindEth/stellar-risc0-verifier:
#     git clone https://github.com/NethermindEth/stellar-risc0-verifier
#     cd stellar-risc0-verifier
#     ./scripts/manage.sh deploy-router   -n testnet -a deployer --min-delay 0
#     ./scripts/manage.sh deploy-verifier -n testnet -a deployer            # prints SELECTOR
#     ./scripts/manage.sh schedule-add-verifier --selector <SELECTOR>
#     ./scripts/manage.sh execute-add-verifier  --selector <SELECTOR>
# then pass the router address here:
#     VERIFIER_ROUTER=C... ./scripts/setup-testnet.sh
#
# Requirements: stellar-cli, python3, cargo (+ wasm32v1-none target), and a real
# out/proof.json (this script generates one if missing).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$ROOT_DIR"

NETWORK="${NETWORK:-testnet}"
SNAPSHOT="${SNAPSHOT:-1700000000}"
RESERVE_STROOPS="${RESERVE_STROOPS:-5000000000}"   # 500.0000000 USDC (>= 4,466,750,000 liabilities)
VERIFIER_ROUTER="${VERIFIER_ROUTER:-}"

if [[ -z "$VERIFIER_ROUTER" ]]; then
  echo "error: set VERIFIER_ROUTER=<router contract id> (see header for how to deploy it)" >&2
  exit 1
fi

j() { python3 -c "import json,sys; print(json.load(open(sys.argv[1]))$2)" "$1"; }

echo "==> 1/8  generating + funding accounts (admin/deployer, issuer, reserve)"
for k in deployer issuer reserve; do
  stellar keys generate "$k" --network "$NETWORK" --fund --overwrite >/dev/null
done
ADMIN="$(stellar keys address deployer)"
ISSUER="$(stellar keys address issuer)"
RESERVE="$(stellar keys address reserve)"
echo "    admin=$ADMIN"
echo "    issuer=$ISSUER"
echo "    reserve=$RESERVE"

echo "==> 2/8  reserve trusts USDC:issuer"
stellar tx new change-trust --source reserve --network "$NETWORK" --line "USDC:$ISSUER" >/dev/null

echo "==> 3/8  issuer pays $RESERVE_STROOPS stroops USDC to reserve"
stellar tx new payment --source issuer --network "$NETWORK" \
  --destination "$RESERVE" --asset "USDC:$ISSUER" --amount "$RESERVE_STROOPS" >/dev/null

echo "==> 4/8  deploying USDC Stellar Asset Contract (SAC)"
USDC_SAC="$(stellar contract asset deploy --asset "USDC:$ISSUER" --source deployer --network "$NETWORK" 2>/dev/null | tail -1)"
echo "    usdc_sac=$USDC_SAC"

echo "==> 5/8  generating proof (if out/proof.json is missing)"
if [[ ! -f out/proof.json ]]; then
  cargo run -p host -- --balances data/mock-balances.json --snapshot "$SNAPSHOT"
fi
IMAGE_ID="$(j out/proof.json "['image_id_hex']")"
echo "    image_id=$IMAGE_ID"

echo "==> 6/8  building attestation contract wasm"
cargo build -p attestation --target wasm32v1-none --release >/dev/null 2>&1
WASM="contracts/attestation/target/wasm32v1-none/release/attestation.wasm"

echo "==> 7/8  deploying attestation contract"
ATTESTATION="$(stellar contract deploy --wasm "$WASM" --source deployer --network "$NETWORK" -- \
  --admin "$ADMIN" --image_id "$IMAGE_ID" --reserve "$RESERVE" \
  --usdc_sac "$USDC_SAC" --verifier_router "$VERIFIER_ROUTER" 2>/dev/null | tail -1)"
echo "    attestation=$ATTESTATION"

echo "==> 8/8  writing deployment.json"
python3 - "$NETWORK" "$SNAPSHOT" "$ISSUER" "$IMAGE_ID" "$ATTESTATION" "$USDC_SAC" "$VERIFIER_ROUTER" "$ADMIN" "$RESERVE" <<'PY'
import json, sys
( _, network, snapshot, issuer, image_id, attestation, usdc_sac, router, admin, reserve ) = sys.argv
json.dump({
  "network": network,
  "snapshot": int(snapshot),
  "asset": f"USDC:{issuer}",
  "image_id": image_id,
  "contracts": {"attestation": attestation, "usdc_sac": usdc_sac, "verifier_router": router},
  "accounts": {"admin": admin, "issuer": issuer, "reserve": reserve},
}, open("deployment.json", "w"), indent=2)
open("deployment.json","a").write("\n")
PY

echo ""
echo "Done. Submit the proof and view the attestation:"
echo "    ./scripts/demo.sh"
