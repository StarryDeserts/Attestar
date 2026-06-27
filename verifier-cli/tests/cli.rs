use std::process::Command;

#[test]
fn verifies_a_known_good_proof() {
    use por_core::{MerkleSumTree, Account};
    let accounts = vec![Account { id: 7, balance: 5 }, Account { id: 8, balance: 9 }];
    let tree = MerkleSumTree::build(&accounts).unwrap();
    let proof = tree.inclusion_proof(0).unwrap();
    let dir = std::env::temp_dir();
    let pf = dir.join("por_proof_test.json");
    std::fs::write(&pf, serde_json::to_vec(&proof).unwrap()).unwrap();
    let root_hex = hex::encode(tree.root());

    let out = Command::new(env!("CARGO_BIN_EXE_verifier-cli"))
        .args(["--proof", pf.to_str().unwrap(), "--root", &root_hex])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("INCLUDED"));
}
