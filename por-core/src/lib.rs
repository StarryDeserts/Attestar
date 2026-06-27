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
}
