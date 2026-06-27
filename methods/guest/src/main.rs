#![no_main]
risc0_zkvm::guest::entry!(main);
use risc0_zkvm::guest::env;
use por_core::{encode_journal, Account, Journal, MerkleSumTree};

fn main() {
    let accounts: Vec<Account> = env::read();
    let snapshot: u64 = env::read();

    let tree = MerkleSumTree::build(&accounts).expect("invalid balance set");
    let journal = Journal {
        root: tree.root(),
        total: tree.total(),
        snapshot,
        count: accounts.len() as u32,
    };
    env::commit_slice(&encode_journal(&journal));
}
