use anyhow::{anyhow, Result};
use clap::Parser;
use por_core::{verify_inclusion, InclusionProof};
use std::{fs, path::PathBuf, process::exit};

#[derive(Parser)]
struct Args {
    /// Path to an inclusion proof JSON (out/inclusion/<id>.json).
    #[arg(long)]
    proof: PathBuf,
    /// The published Merkle root, hex (out/proof.json root_hex / on-chain attestation root).
    #[arg(long)]
    root: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let proof: InclusionProof = serde_json::from_slice(&fs::read(&args.proof)?)?;
    let root_bytes = hex::decode(args.root.trim_start_matches("0x"))?;
    let root: [u8; 32] = root_bytes.try_into().map_err(|_| anyhow!("root must be 32 bytes"))?;

    if verify_inclusion(&proof, &root) {
        println!("INCLUDED  id={} balance={} stroops", proof.id, proof.balance);
        Ok(())
    } else {
        println!("NOT INCLUDED  id={} (proof does not match root)", proof.id);
        exit(1);
    }
}
