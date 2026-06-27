use anyhow::{Context, Result};
use clap::Parser;
use methods::{POR_GUEST_ELF, POR_GUEST_ID};
use por_core::{decode_journal, Account, MerkleSumTree};
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[derive(Parser)]
struct Args {
    /// Path to the mock balances JSON.
    #[arg(long, default_value = "data/mock-balances.json")]
    balances: PathBuf,
    /// Snapshot timestamp T (unix seconds).
    #[arg(long, default_value_t = 1_700_000_000)]
    snapshot: u64,
    /// Output directory.
    #[arg(long, default_value = "out")]
    out: PathBuf,
}

#[derive(Serialize)]
struct ProofFile {
    seal_hex: String,
    image_id_hex: String,
    journal_digest_hex: String,
    root_hex: String,
    total: u64,
    snapshot: u64,
    count: u32,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let accounts: Vec<Account> =
        serde_json::from_slice(&fs::read(&args.balances)?).context("reading balances")?;

    // Build the tree off-zkVM too, so we can emit inclusion proofs.
    let tree = MerkleSumTree::build(&accounts).context("building tree")?;

    let env = ExecutorEnv::builder()
        .write(&accounts)?
        .write(&args.snapshot)?
        .build()?;
    let receipt = default_prover()
        .prove_with_opts(env, POR_GUEST_ELF, &ProverOpts::groth16())?
        .receipt;

    let seal = encode_seal(&receipt)?;
    let journal_digest: [u8; 32] = Sha256::digest(&receipt.journal.bytes).into();
    let mut image_id = [0u8; 32];
    for (i, word) in POR_GUEST_ID.iter().enumerate() {
        image_id[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
    }
    let journal = decode_journal(&receipt.journal.bytes)?;

    fs::create_dir_all(args.out.join("inclusion"))?;
    let proof = ProofFile {
        seal_hex: hex::encode(&seal),
        image_id_hex: hex::encode(image_id),
        journal_digest_hex: hex::encode(journal_digest),
        root_hex: hex::encode(journal.root),
        total: journal.total,
        snapshot: journal.snapshot,
        count: journal.count,
    };
    fs::write(args.out.join("proof.json"), serde_json::to_vec_pretty(&proof)?)?;

    for i in 0..accounts.len() {
        let p = tree.inclusion_proof(i)?;
        fs::write(
            args.out.join("inclusion").join(format!("{}.json", p.id)),
            serde_json::to_vec_pretty(&p)?,
        )?;
    }

    println!("wrote {}/proof.json  (total={} stroops, count={})", args.out.display(), proof.total, proof.count);
    Ok(())
}
