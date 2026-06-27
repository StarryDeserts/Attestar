//
// Headless self-test for por-verify.js — the module the browser dashboard runs.
//
// The point: prove the in-browser inclusion check agrees BYTE-FOR-BYTE with the
// Rust prover (por-core). If computeRoot() on a real exported proof folds to the
// exact root that was committed on-chain, the JS hashing is correct; a tampered
// proof must fail. Run:  node --test frontend/por-verify.test.mjs
//
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import PorVerify from "./por-verify.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

// The Merkle-sum root committed on-chain for snapshot 1700000000 (see deployment.json).
const ON_CHAIN_ROOT =
  "b9936202488d359e83f52c6127f4c58c1876329cba5085a8a5c9220e11c7c59d";

const proofPath = resolve(__dirname, "../out/inclusion/1001.json");
const proof = JSON.parse(readFileSync(proofPath, "utf8"));

test("computeRoot folds the real proof to the on-chain root", async () => {
  const got = await PorVerify.computeRoot(proof);
  assert.equal(got, ON_CHAIN_ROOT);
});

test("verifyInclusion accepts the real proof against the on-chain root", async () => {
  assert.equal(await PorVerify.verifyInclusion(proof, ON_CHAIN_ROOT), true);
});

test("a tampered balance is rejected", async () => {
  const bad = JSON.parse(JSON.stringify(proof));
  bad.balance = Number(bad.balance) + 1;
  assert.equal(await PorVerify.verifyInclusion(bad, ON_CHAIN_ROOT), false);
});

test("a tampered sibling sum is rejected", async () => {
  const bad = JSON.parse(JSON.stringify(proof));
  bad.siblings[0].sum = Number(bad.siblings[0].sum) + 1;
  assert.equal(await PorVerify.verifyInclusion(bad, ON_CHAIN_ROOT), false);
});

test("a flipped sibling byte is rejected", async () => {
  const bad = JSON.parse(JSON.stringify(proof));
  bad.siblings[0].hash[0] = (bad.siblings[0].hash[0] + 1) & 0xff;
  assert.equal(await PorVerify.verifyInclusion(bad, ON_CHAIN_ROOT), false);
});
