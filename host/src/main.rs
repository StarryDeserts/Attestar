use anyhow::Result;
use methods::{POR_GUEST_ELF, POR_GUEST_ID};
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts};
use sha2::{Digest, Sha256};

fn main() -> Result<()> {
    let env = ExecutorEnv::builder().write(&7u64)?.build()?;
    let prove_info = default_prover().prove_with_opts(env, POR_GUEST_ELF, &ProverOpts::groth16())?;
    let receipt = prove_info.receipt;

    let seal = encode_seal(&receipt)?;
    let journal_digest: [u8; 32] = Sha256::digest(&receipt.journal.bytes).into();
    let mut image_id = [0u8; 32];
    for (i, word) in POR_GUEST_ID.iter().enumerate() {
        image_id[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
    }

    println!("seal     {}", hex::encode(&seal));
    println!("image_id {}", hex::encode(image_id));
    println!("journal  {}", hex::encode(journal_digest));
    Ok(())
}
