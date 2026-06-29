"use strict";
//
// Attestar dashboard logic. Two real jobs (no mocks):
//   1. read the live solvency attestation from the Soroban contract (read-only
//      simulate) and render it in whatever state it's in
//      (loading / solvent / under-reserved / unavailable);
//   2. verify a user's Merkle-sum inclusion proof entirely in-browser against
//      the on-chain root, using por-verify.js — the SAME hashing as por-core
//      (Rust), guarded by frontend/por-verify.test.mjs.
//
const $ = (id) => document.getElementById(id);
const { toHex, computeRoot } = window.PorVerify;

const STROOPS = 10_000_000n;
const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------- formatting ----------
function usdc(stroops) {
  const v = BigInt(stroops);
  const whole = v / STROOPS;
  const frac = (v % STROOPS).toString().padStart(7, "0");
  return `${whole.toLocaleString("en-US")}.${frac} USDC`;
}
const usdcNum = (stroops) => Number(BigInt(stroops)) / 1e7;
const shortHex = (h) => (h && h.length > 20 ? `${h.slice(0, 8)}…${h.slice(-8)}` : h);
const shortAddr = (a) => (a && a.length > 16 ? `${a.slice(0, 8)}…${a.slice(-4)}` : a);

// ---------- state ----------
let CFG = null;
let ON_CHAIN_ROOT = null; // hex of the live attestation root

// ---------- config + live read ----------
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

// ---------- shared bits ----------
async function copyText(txt) {
  try {
    await navigator.clipboard.writeText(txt);
  } catch (e) {
    const t = document.createElement("textarea");
    t.value = txt;
    document.body.appendChild(t);
    t.select();
    document.execCommand("copy");
    t.remove();
  }
}
function flashCopy(btn, label) {
  btn.classList.add("copied");
  label.textContent = "Copied";
  setTimeout(() => {
    btn.classList.remove("copied");
    label.textContent = "Copy";
  }, 1600);
}
function animateCount(el) {
  const to = parseFloat(el.dataset.to);
  const dec = parseInt(el.dataset.dec, 10);
  const group = el.dataset.group === "true";
  const fmt = (v) =>
    Number(v).toLocaleString("en-US", { minimumFractionDigits: dec, maximumFractionDigits: dec, useGrouping: group });
  if (reduceMotion) { el.textContent = fmt(to); return; }
  const dur = 1100;
  const start = performance.now();
  (function frame(now) {
    const p = Math.min((now - start) / dur, 1);
    const eased = 1 - Math.pow(1 - p, 3);
    el.textContent = fmt(to * eased);
    if (p < 1) requestAnimationFrame(frame);
  })(start);
}

// ---------- icons ----------
const SH_OK = `<svg class="shield" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 2l8 4v6c0 5-3.4 8.4-8 10-4.6-1.6-8-5-8-10V6l8-4z" fill="currentColor" opacity=".16"/><path d="M12 2l8 4v6c0 5-3.4 8.4-8 10-4.6-1.6-8-5-8-10V6l8-4z" stroke="currentColor" stroke-width="1.4"/><path d="M8.5 12.4l2.3 2.3 4.7-5" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const SH_WARN = `<svg class="shield" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 2l8 4v6c0 5-3.4 8.4-8 10-4.6-1.6-8-5-8-10V6l8-4z" fill="currentColor" opacity=".16"/><path d="M12 2l8 4v6c0 5-3.4 8.4-8 10-4.6-1.6-8-5-8-10V6l8-4z" stroke="currentColor" stroke-width="1.4"/><path d="M12 8v5M12 16h.01" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>`;
const SH_ERR = `<svg class="shield" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 2l8 4v6c0 5-3.4 8.4-8 10-4.6-1.6-8-5-8-10V6l8-4z" stroke="currentColor" stroke-width="1.4" opacity=".7"/><path d="M9.5 9.5l5 5M14.5 9.5l-5 5" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>`;
const IC_COPY = `<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><rect x="9" y="9" width="11" height="11" rx="2" stroke="currentColor" stroke-width="1.6"/><path d="M5 15V5a2 2 0 012-2h8" stroke="currentColor" stroke-width="1.6"/></svg>`;
const IC_EXT = `<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M7 17L17 7M9 7h8v8" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const IC_INFO = `<svg viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 8v5M12 16h.01" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><circle cx="12" cy="12" r="9" stroke="currentColor" stroke-width="1.5"/></svg>`;
const RES_OK = `<svg viewBox="0 0 48 48" aria-hidden="true"><circle class="check-circle" cx="24" cy="24" r="19"/><path class="check-tick" d="M15 24.5l6 6 12-13"/></svg>`;
const RES_ERR = `<svg viewBox="0 0 48 48" aria-hidden="true"><circle cx="24" cy="24" r="19" fill="none" stroke="var(--err)" stroke-width="2.4"/><path d="M17 17l14 14M31 17L17 31" stroke="var(--err)" stroke-width="2.8" stroke-linecap="round"/></svg>`;
const MERKLE_SVG = `<svg viewBox="0 0 320 120" preserveAspectRatio="xMidYMid meet" class="mtree" aria-hidden="true">
  <g class="m-edges">
    <path d="M40 100 L80 64"/><path d="M80 100 L80 64"/><path d="M120 100 L120 64"/><path d="M160 100 L120 64"/>
    <path d="M200 100 L220 64"/><path d="M240 100 L220 64"/><path d="M280 100 L280 64"/>
    <path d="M80 64 L130 30"/><path d="M120 64 L130 30"/><path d="M220 64 L210 30"/><path d="M280 64 L210 30"/>
    <path d="M130 30 L170 12"/><path d="M210 30 L170 12"/>
  </g>
  <g class="m-leaf"><circle cx="40" cy="100" r="5"/><circle cx="80" cy="100" r="5"/><circle cx="120" cy="100" r="5"/><circle cx="160" cy="100" r="5"/><circle cx="200" cy="100" r="5"/><circle cx="240" cy="100" r="5"/><circle cx="280" cy="100" r="5"/></g>
  <g class="m-node"><circle cx="80" cy="64" r="5"/><circle cx="120" cy="64" r="5"/><circle cx="220" cy="64" r="5"/><circle cx="280" cy="64" r="5"/><circle cx="130" cy="30" r="5"/><circle cx="210" cy="30" r="5"/></g>
  <circle class="m-root" cx="170" cy="12" r="7"/>
</svg>`;

// ---------- solvency render-by-state ----------
function renderLoading() {
  $("solvencyMount").innerHTML = `
    <div class="card solvency loading">
      <div class="sk sk-badge"></div>
      <div class="stat-row"><div class="sk-tile"></div><div class="sk-tile"></div><div class="sk-tile"></div><div class="sk-tile"></div></div>
      <div class="sk sk-root"></div>
      <div class="reading-line"><span class="dot"></span> Reading live attestation…</div>
    </div>`;
}

function renderSolvencyError(msg) {
  $("solvencyMount").innerHTML = `
    <div class="card solvency err">
      <div class="status-badge err" role="status">${SH_ERR}<span class="label">— attestation unavailable</span></div>
      <p class="err-line"><b>Could not read live attestation:</b> ${msg}</p>
      <div class="err-hint">${IC_INFO} Solvency cannot be asserted until the chain is reachable. No figures shown — nothing is implied.</div>
    </div>`;
}

function renderSolvency(att) {
  const reserves = BigInt(att.reserves);
  const liabilities = BigInt(att.liabilities);
  const r = usdcNum(reserves);
  const l = usdcNum(liabilities);
  const solvent = !!att.solvent;
  const count = Number(att.count);

  const coverageNum = l > 0 ? (r / l) * 100 : Infinity;
  const covFinite = isFinite(coverageNum);
  const covPctText = covFinite ? `${coverageNum.toFixed(2)}% coverage` : "∞ coverage";
  const covTile = covFinite
    ? `<span class="count" data-to="${coverageNum}" data-dec="2">0.00</span><span class="t-unit">%</span>`
    : `∞`;

  const max = Math.max(r, l) || 1;
  const fillW = Math.min((l / max) * 100, 100);
  const threshW = Math.min((r / max) * 100, 100);

  const warn = !solvent ? " warn" : "";
  const badge = solvent
    ? `<div class="status-badge" role="status">${SH_OK}<span class="label">FULLY RESERVED <span class="sub">solvency attested</span></span></div>`
    : `<div class="status-badge warn" role="status">${SH_WARN}<span class="label">UNDER-RESERVED <span class="sub">coverage below 100%</span></span></div>`;

  const fullRoot = ON_CHAIN_ROOT;
  const contractUrl = `${CFG.explorer_base}/contract/${CFG.attestation_contract}`;

  $("solvencyMount").innerHTML = `
    <div class="card solvency${warn}">
      ${badge}
      <div class="stat-row">
        <div class="tile"><div class="t-label">Reserves · live USDC</div><div class="t-val"><span class="count" data-to="${r}" data-dec="7">0.0000000</span><span class="t-unit">USDC</span></div><div class="t-foot">on-chain balance</div></div>
        <div class="tile"><div class="t-label">Liabilities · proven</div><div class="t-val"><span class="count" data-to="${l}" data-dec="7">0.0000000</span><span class="t-unit">USDC</span></div><div class="t-foot">sum of all balances</div></div>
        <div class="tile is-coverage${warn}"><div class="t-label">Coverage</div><div class="t-val">${covTile}</div><div class="t-foot">reserves ÷ liabilities</div></div>
        <div class="tile"><div class="t-label">Users counted</div><div class="t-val"><span class="count" data-to="${count}" data-dec="0">0</span></div><div class="t-foot">leaves in Merkle-sum tree</div></div>
      </div>

      <div class="coverage${warn}">
        <div class="cov-head"><span class="lab">Reserves vs. liabilities</span><span class="pct">${covPctText}</span></div>
        <div class="cov-track" role="img" aria-label="Liabilities ${usdc(liabilities)} against reserves ${usdc(reserves)} — ${covPctText}.">
          <div class="cov-fill" id="covFill"></div>
          <div class="cov-threshold" id="covThreshold"></div>
        </div>
        <div class="cov-legend">
          <span><span class="swatch" style="background:var(--${solvent ? "ok" : "warn"})"></span> Liabilities · ${usdcNum(liabilities).toLocaleString("en-US", { minimumFractionDigits: 7, maximumFractionDigits: 7 })} (fill)</span>
          <span><span class="line"></span> Reserves 100% · ${usdcNum(reserves).toLocaleString("en-US", { minimumFractionDigits: 7, maximumFractionDigits: 7 })} (line)</span>
        </div>
      </div>

      <div class="merkle-viz">
        ${MERKLE_SVG}
        <div class="merkle-cap"><span class="num">${count}</span> user leaves &middot; Merkle-sum tree &middot; <span style="color:var(--violet)">1 root</span></div>
      </div>

      <div class="root-row">
        <div class="root-box">
          <div class="lbl">Merkle-sum root</div>
          <div class="root-val">
            <code id="rootCode" title="${fullRoot}">${shortHex(fullRoot)}</code>
            <button class="copy-btn" id="copyRoot" type="button" aria-label="Copy full Merkle-sum root">${IC_COPY}<span id="copyLabel">Copy</span></button>
          </div>
        </div>
        <a class="contract-link" id="contractLink" href="${contractUrl}" target="_blank" rel="noopener">View attestation contract ${IC_EXT}</a>
      </div>

      <div class="contract-row">
        <span class="lbl">Soroban contract</span>
        <code class="contract-addr" id="contractAddr" title="${CFG.attestation_contract}">${shortAddr(CFG.attestation_contract)}</code>
        <button class="copy-btn" id="copyContract" type="button" aria-label="Copy contract address">${IC_COPY}<span id="copyContractLabel">Copy</span></button>
      </div>
    </div>`;

  // count-up
  $("solvencyMount").querySelectorAll(".count").forEach(animateCount);
  // coverage bars (animate from 0)
  requestAnimationFrame(() => {
    $("covFill").style.width = fillW + "%";
    $("covThreshold").style.width = threshW + "%";
  });
  // copy buttons
  $("copyRoot").addEventListener("click", async () => { await copyText(fullRoot); flashCopy($("copyRoot"), $("copyLabel")); });
  $("copyContract").addEventListener("click", async () => { await copyText(CFG.attestation_contract); flashCopy($("copyContract"), $("copyContractLabel")); });
}

// ---------- inclusion verify ----------
function showResult(ok, head, sub) {
  const result = $("result");
  result.className = "result";
  void result.offsetWidth; // restart the entry animation
  $("resultIcon").innerHTML = ok ? RES_OK : RES_ERR;
  $("resultHead").textContent = head;
  $("resultSub").textContent = sub;
  result.classList.add("show", ok ? "ok" : "err");
}

async function onVerify() {
  const raw = $("proofInput").value.trim();
  if (!raw) return showResult(false, "NO PROOF PROVIDED", "Load the sample or paste an inclusion proof to verify.");
  if (!ON_CHAIN_ROOT) return showResult(false, "ON-CHAIN ROOT NOT LOADED", "The live attestation hasn't loaded yet — wait for the solvency card above.");
  let proof;
  try {
    proof = JSON.parse(raw);
  } catch (e) {
    return showResult(false, "INVALID PROOF JSON", e.message);
  }
  const btn = $("verifyBtn");
  const lab = $("verifyLabel");
  btn.disabled = true;
  btn.classList.add("is-loading");
  lab.textContent = "Verifying…";
  try {
    const computed = await computeRoot(proof);
    await sleep(reduceMotion ? 0 : 450);
    if (computed === ON_CHAIN_ROOT) {
      showResult(true, `INCLUDED — id ${proof.id}, balance ${usdc(proof.balance)} (verified against on-chain root)`, "Leaf hashes up to the published Merkle-sum root.");
    } else {
      showResult(false, "NOT INCLUDED — this proof does not hash to the on-chain root", `Recomputed ${shortHex(computed)} ≠ on-chain ${shortHex(ON_CHAIN_ROOT)}.`);
    }
  } catch (e) {
    showResult(false, "INVALID PROOF", (e && e.message) || "Could not verify this proof.");
  } finally {
    btn.disabled = false;
    btn.classList.remove("is-loading");
    lab.textContent = "Verify against on-chain root";
  }
}

async function loadSample() {
  const candidates = ["../out/inclusion/1001.json", "out/inclusion/1001.json", "/out/inclusion/1001.json"];
  for (const url of candidates) {
    try {
      const r = await fetch(url, { cache: "no-store" });
      if (r.ok) {
        $("proofInput").value = JSON.stringify(await r.json(), null, 2);
        $("fileName").textContent = "1001.json";
        $("result").className = "result";
        return;
      }
    } catch (_) { /* try next */ }
  }
  showResult(false, "SAMPLE UNAVAILABLE", "Could not load out/inclusion/1001.json — use the file picker or paste a proof.");
}

function onFile(ev) {
  const f = ev.target.files && ev.target.files[0];
  if (!f) return;
  $("fileName").textContent = f.name;
  $("result").className = "result";
  const reader = new FileReader();
  reader.onload = () => { $("proofInput").value = reader.result; };
  reader.onerror = () => showResult(false, "FILE COULD NOT BE READ", "Try choosing the proof file again, or paste its JSON.");
  reader.readAsText(f);
}

// ---------- staggered entry reveal ----------
function runReveal() {
  const items = [
    document.querySelector(".site-head"),
    ...document.querySelectorAll(".section"),
    document.querySelector(".site-foot"),
  ].filter(Boolean);
  document.body.classList.add("reveal-ready");
  if (reduceMotion) { items.forEach((el) => el.classList.add("reveal-in")); return; }
  requestAnimationFrame(() => requestAnimationFrame(() => {
    items.forEach((el, i) => setTimeout(() => el.classList.add("reveal-in"), i * 90));
  }));
}

// ---------- boot ----------
(async function main() {
  $("verifyBtn").addEventListener("click", onVerify);
  $("loadSample").addEventListener("click", loadSample);
  $("proofFile").addEventListener("change", onFile);
  $("footRepo").addEventListener("click", (e) => e.preventDefault());

  renderLoading();
  runReveal();

  try {
    CFG = await loadConfig();
    $("net").textContent = CFG.network;
    $("snap").textContent = String(CFG.snapshot);
    $("footContract").href = `${CFG.explorer_base}/contract/${CFG.attestation_contract}`;
    if (CFG.verifier_contract) $("footVerifier").href = `${CFG.explorer_base}/contract/${CFG.verifier_contract}`;
    else $("footVerifier").setAttribute("aria-disabled", "true");

    const att = await loadAttestation();
    ON_CHAIN_ROOT = toHex(Uint8Array.from(att.root));
    renderSolvency(att);
  } catch (e) {
    renderSolvencyError((e && e.message) || String(e));
  }
})();
