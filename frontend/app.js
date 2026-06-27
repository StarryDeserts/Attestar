"use strict";
//
// Functional dashboard logic. Two jobs:
//   1. read the live solvency attestation from the Soroban contract (read-only simulate)
//   2. verify a user's Merkle-sum inclusion proof entirely in-browser, against the
//      on-chain root, using the SAME hashing as por-core (Rust):
//         leaf : sha256( 0x00 || id_le_u64 || balance_le_u64 )
//         node : sha256( 0x01 || left_hash || right_hash || left_sum_le_u64 || right_sum_le_u64 )
//
const $ = (id) => document.getElementById(id);

// ---------- shared verification ----------
// toHex + verifyInclusion come from por-verify.js — the SAME module the Node
// self-test (por-verify.test.mjs) exercises, so the browser runs exactly what
// is tested against the Rust prover's root.
const { toHex, verifyInclusion } = window.PorVerify;

// ---------- formatting ----------
const STROOPS = 10_000_000n;
function usdc(stroopsBig) {
  const v = BigInt(stroopsBig);
  const whole = v / STROOPS;
  const frac = (v % STROOPS).toString().padStart(7, "0");
  return `${whole.toLocaleString("en-US")}.${frac} USDC`;
}

// ---------- state ----------
let CFG = null;
let ON_CHAIN_ROOT = null; // hex string from the live attestation

// ---------- live attestation ----------
async function loadConfig() {
  const res = await fetch("config.json", { cache: "no-store" });
  if (!res.ok) throw new Error(`config.json ${res.status}`);
  return res.json();
}

async function loadAttestation() {
  const SDK = window.StellarSdk;
  if (!SDK) throw new Error("Stellar SDK failed to load");
  const RPC = SDK.rpc || SDK.SorobanRpc;
  const server = new RPC.Server(CFG.rpcUrl, { allowHttp: CFG.rpcUrl.startsWith("http://") });
  const source = await server.getAccount(CFG.source_account);
  const contract = new SDK.Contract(CFG.attestation_contract);
  const tx = new SDK.TransactionBuilder(source, {
    fee: SDK.BASE_FEE,
    networkPassphrase: CFG.networkPassphrase,
  })
    .addOperation(contract.call("get_attestation", SDK.nativeToScVal(BigInt(CFG.snapshot), { type: "u64" })))
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  if (RPC.Api.isSimulationError(sim)) throw new Error(sim.error || "simulation failed");
  const scv = sim.result && sim.result.retval;
  const att = scv ? SDK.scValToNative(scv) : null;
  if (!att) throw new Error("no attestation stored for this snapshot");
  return att;
}

function renderAttestation(att) {
  const reserves = BigInt(att.reserves);
  const liabilities = BigInt(att.liabilities);
  ON_CHAIN_ROOT = toHex(Uint8Array.from(att.root));

  const badge = $("statusBadge");
  if (att.solvent) {
    badge.textContent = "✅ FULLY RESERVED";
    badge.className = "status ok";
  } else {
    badge.textContent = "⚠️ UNDER-RESERVED";
    badge.className = "status warn";
  }
  $("reserves").textContent = usdc(reserves);
  $("liabilities").textContent = usdc(liabilities);
  const pct = liabilities === 0n ? "∞" : `${(Number(reserves * 10000n / liabilities) / 100).toFixed(2)}%`;
  $("coverage").textContent = pct;
  $("count").textContent = String(att.count);
  $("root").textContent = ON_CHAIN_ROOT;
  $("snap").textContent = String(CFG.snapshot);

  const link = $("contractLink");
  link.href = `${CFG.explorer_base}/contract/${CFG.attestation_contract}`;
}

function showSolvencyError(msg) {
  $("statusBadge").textContent = "— attestation unavailable";
  $("statusBadge").className = "status err";
  const el = $("solvencyError");
  el.textContent = `Could not read live attestation: ${msg}`;
  el.hidden = false;
}

// ---------- inclusion UI ----------
function parseProof() {
  const raw = $("proofInput").value.trim();
  if (!raw) throw new Error("paste an inclusion proof first");
  return JSON.parse(raw);
}

async function onVerify() {
  const out = $("inclusionResult");
  out.hidden = false;
  out.className = "result";
  try {
    if (!ON_CHAIN_ROOT) throw new Error("on-chain root not loaded yet");
    const proof = parseProof();
    const ok = await verifyInclusion(proof, ON_CHAIN_ROOT);
    if (ok) {
      out.className = "result ok";
      out.textContent = `✅ INCLUDED — id ${proof.id}, balance ${usdc(proof.balance)} (verified against on-chain root)`;
    } else {
      out.className = "result err";
      out.textContent = `❌ NOT INCLUDED — this proof does not hash to the on-chain root`;
    }
  } catch (e) {
    out.className = "result err";
    out.textContent = `❌ ${e.message}`;
  }
}

async function loadSample() {
  const candidates = ["../out/inclusion/1001.json", "out/inclusion/1001.json", "/out/inclusion/1001.json"];
  for (const url of candidates) {
    try {
      const r = await fetch(url, { cache: "no-store" });
      if (r.ok) { $("proofInput").value = JSON.stringify(await r.json(), null, 2); return; }
    } catch (_) { /* try next */ }
  }
  $("inclusionResult").hidden = false;
  $("inclusionResult").className = "result err";
  $("inclusionResult").textContent = "Could not load sample — use the file picker or paste a proof.";
}

function onFile(ev) {
  const f = ev.target.files && ev.target.files[0];
  if (!f) return;
  const reader = new FileReader();
  reader.onload = () => { $("proofInput").value = reader.result; };
  reader.readAsText(f);
}

// ---------- boot ----------
(async function main() {
  $("verifyBtn").addEventListener("click", onVerify);
  $("loadSample").addEventListener("click", loadSample);
  $("proofFile").addEventListener("change", onFile);
  try {
    CFG = await loadConfig();
    $("net").textContent = CFG.network;
    $("snap").textContent = String(CFG.snapshot);
    const att = await loadAttestation();
    renderAttestation(att);
  } catch (e) {
    showSolvencyError(e.message || String(e));
  }
})();
