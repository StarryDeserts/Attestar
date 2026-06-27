"use strict";
//
// Merkle-sum inclusion verification — a byte-for-byte port of por-core (Rust).
// Shared by the browser dashboard (app.js) and the Node self-test (por-verify.test.mjs),
// so what the demo runs is exactly what is tested.
//
//   leaf : sha256( 0x00 || id_le_u64 || balance_le_u64 )
//   node : sha256( 0x01 || left_hash || right_hash || left_sum_le_u64 || right_sum_le_u64 )
//
// verify_inclusion folds the leaf up through the siblings and compares to the root.
//
(function (root) {
  function u64le(n) {
    let x = BigInt(n);
    const b = new Uint8Array(8);
    for (let i = 0; i < 8; i++) { b[i] = Number(x & 0xffn); x >>= 8n; }
    return b;
  }
  function concat(...parts) {
    const arrs = parts.map((p) => (p instanceof Uint8Array ? p : Uint8Array.from(p)));
    const len = arrs.reduce((a, x) => a + x.length, 0);
    const out = new Uint8Array(len);
    let o = 0;
    for (const a of arrs) { out.set(a, o); o += a.length; }
    return out;
  }
  function toHex(bytes) {
    return Array.from(bytes).map((b) => b.toString(16).padStart(2, "0")).join("");
  }
  async function sha256(bytes) {
    const d = await globalThis.crypto.subtle.digest("SHA-256", bytes);
    return new Uint8Array(d);
  }

  // Returns the root hex this proof folds to (independent of any expected root).
  async function computeRoot(proof) {
    if (!proof || typeof proof !== "object") throw new Error("proof is not an object");
    if (!Array.isArray(proof.siblings)) throw new Error("proof.siblings missing");
    let hash = await sha256(concat([0x00], u64le(proof.id), u64le(proof.balance)));
    let sum = BigInt(proof.balance);
    for (const s of proof.siblings) {
      const sib = Uint8Array.from(s.hash);
      const ssum = BigInt(s.sum);
      if (s.is_left) {
        hash = await sha256(concat([0x01], sib, hash, u64le(ssum), u64le(sum)));
      } else {
        hash = await sha256(concat([0x01], hash, sib, u64le(sum), u64le(ssum)));
      }
      sum += ssum;
    }
    return toHex(hash);
  }

  async function verifyInclusion(proof, rootHex) {
    const got = await computeRoot(proof);
    return got === String(rootHex).toLowerCase();
  }

  const api = { u64le, concat, toHex, sha256, computeRoot, verifyInclusion };
  if (typeof module !== "undefined" && module.exports) module.exports = api;
  else root.PorVerify = api;
})(typeof self !== "undefined" ? self : globalThis);
