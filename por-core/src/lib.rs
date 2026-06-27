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
}
