# ZK Proof-of-Reserves — Dashboard Design Brief

A self-contained brief for redesigning the dashboard in **open-design**. The
logic is built, tested, and live on Stellar testnet; only the visual layer is
being replaced. Nothing here changes behavior — it dresses what already works.

- **Part 1** — the functional skeleton that exists today (and the binding
  contract the redesign must keep intact).
- **Part 2** — the screens and states to design, framed around the demo.
- **Part 3** — copy-paste prompts for open-design.

---

## Part 1 — What's already built (and working)

### Stack & architecture

- **Single static page**, no build step: `frontend/index.html` + `app.js` +
  `style.css` + `por-verify.js` + `config.json`. Served by any static server.
- **Stellar JS SDK 13.3.0** (CDN, browser global `StellarSdk`) — read-only.
- **Web Crypto** (`crypto.subtle`) for the in-browser Merkle-sum verification.
- Verified headlessly end-to-end (live read + inclusion check + tamper case),
  zero console errors. The redesign is purely the HTML/CSS skin.

### Two data sources

1. **Live on-chain attestation** — a read-only Soroban `simulateTransaction`
   call to `get_attestation(snapshot)` on the attestation contract. Returns:
   `{ solvent: bool, reserves: i128, liabilities: i128, count: u32, snapshot:
   u64, root: bytes[32] }`. Amounts are in stroops (1 USDC = 10,000,000).
2. **A user's inclusion proof** — a small JSON file (`out/inclusion/<id>.json`)
   the user loads or pastes. Verified entirely in the browser against the
   on-chain `root` — no server, no trust.

### The two functional areas

**A. Solvency attestation (read live, every page load)**
- A headline status: solvent / under-reserved / unavailable.
- Reserves (live USDC held), Liabilities (proven total), Coverage %, Users
  counted, the Merkle-sum root (32-byte hex), the snapshot id.
- A link out to the attestation contract on stellar.expert.

**B. Inclusion verification (interactive)**
- Load a sample proof, pick a file, or paste JSON.
- "Verify against on-chain root" → folds the proof's leaf up through its
  siblings and compares to the live root.
- Result: INCLUDED (with the user's id + balance), NOT INCLUDED, or a parse
  error. Tampering with one digit flips it to NOT INCLUDED.

### Live values right now (testnet, snapshot 1700000000)

Use these verbatim in mockups so they read as real, not lorem:

| Field | Value |
|---|---|
| Status | **FULLY RESERVED** (solvent) |
| Reserves (live USDC) | `500.0000000 USDC` |
| Liabilities (proven) | `446.6750000 USDC` |
| Coverage | `111.93%` |
| Users counted | `7` |
| Merkle-sum root | `b9936202488d359e83f52c6127f4c58c1876329cba5085a8a5c9220e11c7c59d` |
| Snapshot | `1700000000` |
| Network | Stellar **testnet** |
| Attestation contract | `CBMZGJJYJCBNEG3HHPEE42XPP6TNINKWK2SM7XM3H7DNXNAPZXI2ZTBK` |
| Sample inclusion result | `INCLUDED — id 1001, balance 12.5000000 USDC` |

### Functional binding contract — DO NOT break these

`app.js` finds elements by `id` and toggles state classes. Whatever markup
open-design produces, these ids and classes must survive (or we rewire JS — but
keeping them is free). Each id holds exactly the content noted:

| Element id | Holds / does |
|---|---|
| `net` | network name text ("testnet") |
| `snap` | snapshot id text |
| `statusBadge` | the big solvency badge; class toggles `status ok` / `status warn` / `status err` |
| `reserves` | reserves, formatted USDC |
| `liabilities` | liabilities, formatted USDC |
| `coverage` | coverage %, e.g. `111.93%` |
| `count` | users counted |
| `root` | Merkle-sum root, hex (monospace) |
| `contractLink` | `<a>` to the contract on the explorer |
| `solvencyError` | hidden error line, shown only on read failure |
| `proofFile` | `<input type="file">` |
| `loadSample` | "Load sample" button |
| `proofInput` | `<textarea>` for the proof JSON |
| `verifyBtn` | "Verify" button |
| `inclusionResult` | result banner; class toggles `result ok` / `result err`; `hidden` until a result exists |

State classes the CSS must style: `.status.ok/.warn/.err`, `.result.ok/.err`.

---

## Part 2 — Screens & states to design

This is a **single-page** product. "Pages" here means the sections of that page
plus the **states** each interactive area can be in — open-design should treat
each state as its own frame.

### The demo story (so the design serves it)

A 2–3 min narrative: (1) land → "this exchange is **FULLY RESERVED**, and that
claim was verified *on-chain*, not by their word"; (2) point at reserves vs
liabilities + the root + the contract link; (3) "and **you** can check your own
balance was counted" → load proof → **INCLUDED**; (4) tamper one digit →
**NOT INCLUDED** → "real verification, not a screenshot." The visuals must make
beats (1) and (3)/(4) land instantly on screen-record.

### Page inventory (top → bottom, one scroll)

1. **Header / hero** — product name, one-line value prop, a meta strip
   (network · snapshot · a live indicator). Sets the "trust + crypto" tone.
2. **Solvency attestation panel** — the centerpiece. Big status, the stat
   tiles, a coverage visualization (reserves vs liabilities), the root in
   monospace with copy, link to the on-chain contract.
3. **Inclusion verification panel** — the interactive checker: input affordances
   (file / sample / paste), the verify action, the result banner.
4. **How it works** (optional, high judge-value) — a compact horizontal flow of
   the ZK pipeline (see copy in Part 3). Communicates the ZK is load-bearing.
5. **Footer** — credits + links (repo, contract, verifier).

### States to produce as separate frames

- **Solvency panel:** (a) loading, (b) **solvent / healthy** (primary frame),
  (c) under-reserved / warning, (d) unavailable / error.
- **Inclusion panel:** (a) idle / empty, (b) **INCLUDED / success** (primary),
  (c) NOT INCLUDED / fail, (d) parse error.

The two **primary** frames (solvent + INCLUDED) are what the demo dwells on —
give them the most polish.

### Responsive & accessibility

- **Desktop-first** (the demo is recorded on desktop, ~1280px); must stay
  usable down to ~390px mobile (stat grid collapses to one column).
- AA contrast; status must not rely on color alone (icon + label too); visible
  focus rings; the root/address monospace must remain copyable.

---

## Part 3 — open-design prompts

Paste **3a** for a one-shot "design the whole page in all states." Use the
**3b** blocks to iterate on a single frame. **3c** is for when open-design emits
code rather than mockups.

### 3a. Master prompt (product + design system)

```
Design a high-fidelity web dashboard for a "ZK Proof-of-Reserves" product on
the Stellar blockchain.

WHAT IT IS
A crypto exchange proves that its live on-chain USDC reserves fully cover the
sum of every user's balance — verified by a zero-knowledge proof checked inside
a smart contract — without revealing any individual balance. This dashboard
shows that solvency claim, read live from the chain, and lets any user verify
their own balance was included in the proven total, entirely in their browser.

AUDIENCE & TONE
Hackathon judges and crypto-literate users. Tone: trustworthy, precise,
cryptographic — "Stripe meets a block explorer meets a ZK proving system."
Confident, calm, data-forward. Not playful, not enterprise-stuffy.

VISUAL DIRECTION — dark-first, modern fintech/crypto
- Page background near-black ink (#0A0C12); cards #11151E; elevated #161B26;
  hairline borders #232A38.
- Text: #E6EAF2 primary, #9AA4B2 muted, #6B7280 faint.
- Semantic colors (keep their meaning strict):
  - "verified / solvent" green #2FD98A (tint bg #0F2A20)
  - warning amber #F5B14C (tint #2A2110)
  - error red #F26D78 (tint #2A1416)
- Brand accent (interactive, links, the ZK flow): electric violet #7C6CF6 with
  cyan #22D3EE as a secondary highlight. Reserve green ONLY for solvency/success
  so it stays meaningful.
- Typography: geometric grotesk sans (Inter / Geist / Satoshi) for UI; a
  monospace (Geist Mono / JetBrains Mono / IBM Plex Mono) for ALL hashes,
  addresses, and numeric amounts.
- Shape & space: card radius 14–16px, controls 10px; spacing scale
  4/8/12/16/24/32/48; page max-width ~1000px, generous whitespace.
- Motion (subtle, demo-friendly): a pulsing green "live" dot; numbers count up
  on load; a check mark draws in on INCLUDED; a soft accent glow on the
  solvency hero card. Nothing bouncy.

LAYOUT (single page, vertical scroll, max-width ~1000px centered)
1. Header: product name "ZK Proof-of-Reserves", one-line value prop, and a meta
   strip "Stellar testnet · snapshot 1700000000 · ● live".
2. Solvency attestation card (the hero): a large status badge, a 4-tile stat row
   (Reserves, Liabilities, Coverage, Users counted), a coverage visualization
   comparing reserves vs liabilities, the Merkle-sum root in monospace
   (truncated b9936202…11c7c59d with a copy button), and a link
   "View attestation contract ↗".
3. Inclusion verification card: heading "Verify your inclusion", helper text, an
   input row (file picker, "Load sample" button), a monospace textarea for the
   proof JSON, a primary "Verify against on-chain root" button, and a result
   banner below.
4. "How it works" strip: a horizontal 6-step flow of the ZK pipeline.
5. Footer: "Built for Stellar Hacks: Real-World ZK · RISC Zero → Groth16 →
   Soroban", with links: Repo, Contract, Verifier.

REAL CONTENT (use verbatim — do not invent numbers)
- Value prop: "An exchange proves its USDC reserves cover every user's balance —
  verified on-chain by a Soroban contract, without revealing any balance."
- Status (healthy): "FULLY RESERVED"
- Reserves (live USDC): 500.0000000 USDC
- Liabilities (proven): 446.6750000 USDC
- Coverage: 111.93%
- Users counted: 7
- Merkle-sum root: b9936202488d359e83f52c6127f4c58c1876329cba5085a8a5c9220e11c7c59d
- Snapshot: 1700000000
- Contract: CBMZGJJYJCBNEG3HHPEE42XPP6TNINKWK2SM7XM3H7DNXNAPZXI2ZTBK
- Inclusion success: "INCLUDED — id 1001, balance 12.5000000 USDC
  (verified against on-chain root)"
- Inclusion fail: "NOT INCLUDED — this proof does not hash to the on-chain root"

Deliver high-fidelity desktop frames (~1280px) plus a mobile frame (~390px) of
the full page in its healthy/solvent + INCLUDED state, and separate frames for
the states listed below.
```

### 3b. Per-frame prompts (iterate one state at a time)

Reuse the design system from 3a; each block describes one frame.

```
FRAME — Solvency: SOLVENT (primary)
The hero attestation card in its healthy state. Large badge reading
"FULLY RESERVED" with a verified-green treatment and a check/shield icon.
Four stat tiles: Reserves 500.0000000 USDC · Liabilities 446.6750000 USDC ·
Coverage 111.93% · Users counted 7. A coverage visualization: a horizontal bar
or gauge where liabilities is the filled amount and reserves is the (larger)
capacity, a 100% threshold marker, green because reserves ≥ liabilities, labeled
"111.93% coverage". Below: "Merkle-sum root" with monospace value
b9936202…11c7c59d and a copy button, and a link "View attestation contract ↗".
```

```
FRAME — Solvency: LOADING
Same card, skeleton/shimmer placeholders for the badge, the four tiles, and the
root. A small "Reading live attestation…" line with the pulsing live dot.
```

```
FRAME — Solvency: UNDER-RESERVED
Same card but the warning state: amber badge "UNDER-RESERVED", coverage below
100% (show e.g. 92.40% for the mock), the coverage bar amber with liabilities
exceeding the reserves marker. Same tiles/root layout. This is the "honest
failure" look — serious, not alarming-red.
```

```
FRAME — Solvency: UNAVAILABLE
Same card, error state: a muted red "— attestation unavailable" badge and a
single line "Could not read live attestation: <reason>". No stat tiles.
```

```
FRAME — Inclusion: IDLE
The verification card, empty. Heading "Verify your inclusion", helper text
"Paste or load your inclusion proof (e.g. out/inclusion/1001.json).
Verification runs entirely in your browser against the on-chain root." Input row:
a file picker and a secondary "Load sample (1001)" button. An empty monospace
textarea with a JSON placeholder. A primary button "Verify against on-chain root".
```

```
FRAME — Inclusion: INCLUDED (primary)
Same card after a successful verify. The textarea holds pretty-printed proof
JSON. A success banner in verified-green with a drawn-in check:
"INCLUDED — id 1001, balance 12.5000000 USDC (verified against on-chain root)".
```

```
FRAME — Inclusion: NOT INCLUDED
Same card, failure banner in red:
"NOT INCLUDED — this proof does not hash to the on-chain root".
Convey "this is real cryptographic verification" — precise, not a generic error.
```

```
FRAME — How it works (ZK pipeline strip)
A horizontal 6-step flow with small icons and short labels, connected by arrows,
using the violet/cyan brand accents:
1. User balances (private)
2. Merkle-sum tree → public root
3. RISC Zero zkVM proves reserves ≥ Σ balances, each balance in range
4. Groth16 wraps the proof → 260-byte seal
5. Soroban contract verifies the proof + reads live USDC reserves
6. On-chain attestation: solvent ✓
Compact, legible at a glance on a screen recording.
```

### 3c. If open-design outputs code (integration constraints)

```
If you generate HTML/CSS, keep these element ids so existing JavaScript binds to
them (it sets text content and toggles state classes — do not rename):
net, snap, statusBadge, reserves, liabilities, coverage, count, root,
contractLink, solvencyError, proofFile, loadSample, proofInput, verifyBtn,
inclusionResult.
Style these state classes:
- .status.ok (solvent, green), .status.warn (under-reserved, amber),
  .status.err (unavailable, red) — applied to #statusBadge.
- .result.ok (green), .result.err (red) — applied to #inclusionResult, which
  starts with the `hidden` attribute and is revealed when a result exists.
Prefer plain semantic HTML + a single CSS file (no framework/build step) so it
drops into the existing static page. Keep hashes/addresses/amounts in a
monospace font and selectable.
```

---

## Notes for wiring the result back in

open-design produces the look; the existing `app.js` keeps the behavior. Two
clean paths once the design is back:

1. **Reskin in place** — replace `index.html` structure + `style.css`, keep the
   ids/classes above. `app.js` and `por-verify.js` are untouched. Smallest diff.
2. **Port** — if open-design emits a different structure, map its elements to the
   ids above (or update the few `getElementById` calls in `app.js`). The Node
   self-test (`frontend/por-verify.test.mjs`) still guards the crypto.

Either way, re-run the headless checks before recording the demo.
