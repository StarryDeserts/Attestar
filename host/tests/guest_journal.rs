use methods::POR_GUEST_ELF;
use por_core::{decode_journal, Account, MerkleSumTree};
use risc0_zkvm::{default_executor, ExecutorEnv};

#[test]
fn guest_commits_expected_journal() {
    let accounts = vec![
        Account { id: 1, balance: 100 },
        Account { id: 2, balance: 250 },
        Account { id: 3, balance: 0 },
    ];
    let snapshot: u64 = 1_700_000_000;

    let env = ExecutorEnv::builder()
        .write(&accounts).unwrap()
        .write(&snapshot).unwrap()
        .build().unwrap();

    let session = default_executor().execute(env, POR_GUEST_ELF).unwrap();
    let journal = decode_journal(&session.journal.bytes).unwrap();

    let expected = MerkleSumTree::build(&accounts).unwrap();
    assert_eq!(journal.root, expected.root());
    assert_eq!(journal.total, 350);
    assert_eq!(journal.snapshot, snapshot);
    assert_eq!(journal.count, 3);
}
