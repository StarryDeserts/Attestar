#![no_main]
risc0_zkvm::guest::entry!(main);
use risc0_zkvm::guest::env;

fn main() {
    let x: u64 = env::read();
    env::commit_slice(&x.to_le_bytes());
}
