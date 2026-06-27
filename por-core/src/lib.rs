use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: u64,
    pub balance: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PorError {
    Empty,
    Overflow,
    IndexOutOfRange,
    BadJournal,
}

impl core::fmt::Display for PorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            PorError::Empty => "account set is empty",
            PorError::Overflow => "balance sum overflow",
            PorError::IndexOutOfRange => "index out of range",
            PorError::BadJournal => "malformed journal bytes",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for PorError {}

const TAG_LEAF: u8 = 0x00;
const TAG_NODE: u8 = 0x01;
const TAG_PADDING: u8 = 0x02;

pub(crate) fn hash_leaf(id: u64, balance: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([TAG_LEAF]);
    h.update(id.to_le_bytes());
    h.update(balance.to_le_bytes());
    h.finalize().into()
}

pub(crate) fn hash_padding(index: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([TAG_PADDING]);
    h.update(index.to_le_bytes());
    h.finalize().into()
}

pub(crate) fn hash_node(l: &[u8; 32], l_sum: u64, r: &[u8; 32], r_sum: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([TAG_NODE]);
    h.update(l);
    h.update(r);
    h.update(l_sum.to_le_bytes());
    h.update(r_sum.to_le_bytes());
    h.finalize().into()
}

#[derive(Clone, Debug)]
struct Node {
    hash: [u8; 32],
    sum: u64,
}

#[derive(Clone, Debug)]
pub struct MerkleSumTree {
    accounts: Vec<Account>,
    levels: Vec<Vec<Node>>, // levels[0] = padded leaves; last = [root]
}

impl MerkleSumTree {
    pub fn build(accounts: &[Account]) -> Result<MerkleSumTree, PorError> {
        if accounts.is_empty() {
            return Err(PorError::Empty);
        }
        let mut total: u64 = 0;
        for a in accounts {
            total = total.checked_add(a.balance).ok_or(PorError::Overflow)?;
        }

        let n = accounts.len();
        let padded = n.next_power_of_two();
        let mut leaves: Vec<Node> = Vec::with_capacity(padded);
        for a in accounts {
            leaves.push(Node { hash: hash_leaf(a.id, a.balance), sum: a.balance });
        }
        for i in n..padded {
            leaves.push(Node { hash: hash_padding(i as u64), sum: 0 });
        }

        let mut levels = vec![leaves];
        while levels.last().unwrap().len() > 1 {
            let cur = levels.last().unwrap();
            let mut next = Vec::with_capacity(cur.len() / 2);
            for pair in cur.chunks(2) {
                let sum = pair[0].sum + pair[1].sum; // safe: bounded by `total` (already u64-checked)
                let hash = hash_node(&pair[0].hash, pair[0].sum, &pair[1].hash, pair[1].sum);
                next.push(Node { hash, sum });
            }
            levels.push(next);
        }

        Ok(MerkleSumTree { accounts: accounts.to_vec(), levels })
    }

    pub fn root(&self) -> [u8; 32] {
        self.levels.last().unwrap()[0].hash
    }

    pub fn total(&self) -> u64 {
        self.levels.last().unwrap()[0].sum
    }

    pub fn count(&self) -> usize {
        self.accounts.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sibling {
    pub hash: [u8; 32],
    pub sum: u64,
    pub is_left: bool, // true if the sibling sits on the LEFT of the current node
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InclusionProof {
    pub id: u64,
    pub balance: u64,
    pub index: usize,
    pub siblings: Vec<Sibling>,
}

impl MerkleSumTree {
    pub fn inclusion_proof(&self, index: usize) -> Result<InclusionProof, PorError> {
        if index >= self.accounts.len() {
            return Err(PorError::IndexOutOfRange);
        }
        let mut siblings = Vec::new();
        let mut idx = index;
        for level in &self.levels[..self.levels.len() - 1] {
            let sib_idx = idx ^ 1;
            let sib = &level[sib_idx];
            siblings.push(Sibling { hash: sib.hash, sum: sib.sum, is_left: sib_idx < idx });
            idx /= 2;
        }
        let acct = self.accounts[index];
        Ok(InclusionProof { id: acct.id, balance: acct.balance, index, siblings })
    }
}

pub fn verify_inclusion(proof: &InclusionProof, expected_root: &[u8; 32]) -> bool {
    let mut hash = hash_leaf(proof.id, proof.balance);
    let mut sum = proof.balance;
    for s in &proof.siblings {
        if s.is_left {
            hash = hash_node(&s.hash, s.sum, &hash, sum);
        } else {
            hash = hash_node(&hash, sum, &s.hash, s.sum);
        }
        sum = match sum.checked_add(s.sum) {
            Some(v) => v,
            None => return false,
        };
    }
    &hash == expected_root
}

pub const JOURNAL_LEN: usize = 32 + 8 + 8 + 4; // 52

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal {
    pub root: [u8; 32],
    pub total: u64,
    pub snapshot: u64,
    pub count: u32,
}

pub fn encode_journal(j: &Journal) -> [u8; JOURNAL_LEN] {
    let mut out = [0u8; JOURNAL_LEN];
    out[0..32].copy_from_slice(&j.root);
    out[32..40].copy_from_slice(&j.total.to_le_bytes());
    out[40..48].copy_from_slice(&j.snapshot.to_le_bytes());
    out[48..52].copy_from_slice(&j.count.to_le_bytes());
    out
}

pub fn decode_journal(bytes: &[u8]) -> Result<Journal, PorError> {
    if bytes.len() != JOURNAL_LEN {
        return Err(PorError::BadJournal);
    }
    let mut root = [0u8; 32];
    root.copy_from_slice(&bytes[0..32]);
    let total = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
    let snapshot = u64::from_le_bytes(bytes[40..48].try_into().unwrap());
    let count = u32::from_le_bytes(bytes[48..52].try_into().unwrap());
    Ok(Journal { root, total, snapshot, count })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaf_node_padding_are_domain_separated() {
        let a = hash_leaf(1, 100);
        let b = hash_padding(0);
        assert_ne!(a, b);
        let n = hash_node(&a, 100, &b, 0);
        assert_ne!(n, a);
        assert_ne!(n, b);
    }

    #[test]
    fn build_sums_and_is_deterministic() {
        let accounts = vec![
            Account { id: 1, balance: 100 },
            Account { id: 2, balance: 250 },
            Account { id: 3, balance: 0 },
        ];
        let t = MerkleSumTree::build(&accounts).unwrap();
        assert_eq!(t.total(), 350);
        assert_eq!(t.count(), 3);
        let t2 = MerkleSumTree::build(&accounts).unwrap();
        assert_eq!(t.root(), t2.root());
    }

    #[test]
    fn build_rejects_empty() {
        assert_eq!(MerkleSumTree::build(&[]).unwrap_err(), PorError::Empty);
    }

    #[test]
    fn build_rejects_overflow() {
        let accounts = vec![
            Account { id: 1, balance: u64::MAX },
            Account { id: 2, balance: 1 },
        ];
        assert_eq!(MerkleSumTree::build(&accounts).unwrap_err(), PorError::Overflow);
    }

    #[test]
    fn single_account_root_is_leaf_chained_to_self_pow2() {
        let t = MerkleSumTree::build(&[Account { id: 9, balance: 42 }]).unwrap();
        assert_eq!(t.total(), 42);
    }

    #[test]
    fn inclusion_roundtrip_valid() {
        let accounts = vec![
            Account { id: 10, balance: 100 },
            Account { id: 20, balance: 250 },
            Account { id: 30, balance: 5 },
            Account { id: 40, balance: 0 },
            Account { id: 50, balance: 700 },
        ];
        let t = MerkleSumTree::build(&accounts).unwrap();
        let root = t.root();
        for (i, account) in accounts.iter().enumerate() {
            let p = t.inclusion_proof(i).unwrap();
            assert_eq!(p.id, account.id);
            assert_eq!(p.balance, account.balance);
            assert!(verify_inclusion(&p, &root), "proof {i} should verify");
        }
    }

    #[test]
    fn inclusion_rejects_tampered_balance() {
        let accounts = vec![Account { id: 1, balance: 100 }, Account { id: 2, balance: 200 }];
        let t = MerkleSumTree::build(&accounts).unwrap();
        let root = t.root();
        let mut p = t.inclusion_proof(0).unwrap();
        p.balance += 1; // lie
        assert!(!verify_inclusion(&p, &root));
    }

    #[test]
    fn inclusion_rejects_wrong_root() {
        let accounts = vec![Account { id: 1, balance: 100 }, Account { id: 2, balance: 200 }];
        let t = MerkleSumTree::build(&accounts).unwrap();
        let p = t.inclusion_proof(1).unwrap();
        assert!(!verify_inclusion(&p, &[0u8; 32]));
    }

    #[test]
    fn inclusion_index_out_of_range() {
        let t = MerkleSumTree::build(&[Account { id: 1, balance: 1 }]).unwrap();
        assert_eq!(t.inclusion_proof(5).unwrap_err(), PorError::IndexOutOfRange);
    }

    #[test]
    fn journal_encode_layout_is_exact() {
        let j = Journal { root: [0xAB; 32], total: 0x1122334455667788, snapshot: 0x00000000DEADBEEF, count: 0x01020304 };
        let b = encode_journal(&j);
        assert_eq!(b.len(), 52);
        assert_eq!(&b[0..32], &[0xAB; 32]);
        assert_eq!(&b[32..40], &0x1122334455667788u64.to_le_bytes());
        assert_eq!(&b[40..48], &0x00000000DEADBEEFu64.to_le_bytes());
        assert_eq!(&b[48..52], &0x01020304u32.to_le_bytes());
    }

    #[test]
    fn journal_roundtrip() {
        let j = Journal { root: [7; 32], total: 999_999, snapshot: 1_700_000_000, count: 12345 };
        let decoded = decode_journal(&encode_journal(&j)).unwrap();
        assert_eq!(decoded, j);
    }

    #[test]
    fn journal_decode_rejects_wrong_length() {
        assert_eq!(decode_journal(&[0u8; 51]).unwrap_err(), PorError::BadJournal);
    }
}
