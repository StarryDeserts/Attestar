#![cfg(test)]
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contract, contractimpl, symbol_short, token, Address, Bytes, BytesN, Env};
use risc0_interface::VerifierError;
use crate::{Attestation, Error, ProofOfReserves, ProofOfReservesClient};

// --- Mock router: succeeds unless a "reject" flag is set, then it traps. ---
#[contract]
pub struct MockVerifier;

#[contractimpl]
impl MockVerifier {
    pub fn set_reject(env: Env, v: bool) {
        env.storage().instance().set(&symbol_short!("reject"), &v);
    }
    pub fn verify(
        env: Env,
        _seal: Bytes,
        _image_id: BytesN<32>,
        _journal: BytesN<32>,
    ) -> Result<(), VerifierError> {
        let reject: bool = env.storage().instance().get(&symbol_short!("reject")).unwrap_or(false);
        if reject {
            panic!("invalid proof");
        }
        Ok(())
    }
}

struct Fixture {
    env: Env,
    client: ProofOfReservesClient<'static>,
    reserve: Address,
    usdc: Address,
    verifier: Address,
    admin: Address,
}

fn setup(reserve_amount: i128) -> Fixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let reserve = Address::generate(&env);

    // Test SAC + mint reserves to the reserve address.
    let sac_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(sac_admin.clone());
    let usdc = sac.address();
    token::StellarAssetClient::new(&env, &usdc).mint(&reserve, &reserve_amount);

    let verifier = env.register(MockVerifier, ());
    let image_id = BytesN::from_array(&env, &[0u8; 32]);

    let contract_id = env.register(
        ProofOfReserves,
        (admin.clone(), image_id, reserve.clone(), usdc.clone(), verifier.clone()),
    );
    let client = ProofOfReservesClient::new(&env, &contract_id);

    Fixture { env, client, reserve, usdc, verifier, admin }
}

#[test]
fn solvent_attestation_is_stored() {
    let f = setup(500_000_000); // reserves
    let root = BytesN::from_array(&f.env, &[9u8; 32]);
    let seal = Bytes::from_array(&f.env, &[1, 2, 3, 4]);

    let att = f.client.submit_proof(&seal, &root, &400_000_000u64, &1_700_000_000u64, &7u32);
    assert!(att.solvent);
    assert_eq!(att.liabilities, 400_000_000);
    assert_eq!(att.reserves, 500_000_000);

    let stored = f.client.get_attestation(&1_700_000_000u64).unwrap();
    assert_eq!(stored.root, root);
    assert_eq!(stored.solvent, true);
}

#[test]
fn insolvent_is_rejected() {
    let f = setup(100_000_000); // reserves < liabilities
    let root = BytesN::from_array(&f.env, &[9u8; 32]);
    let seal = Bytes::from_array(&f.env, &[1, 2, 3, 4]);

    let res = f.client.try_submit_proof(&seal, &root, &400_000_000u64, &1u64, &7u32);
    assert_eq!(res, Err(Ok(Error::Insolvent)));
    assert!(f.client.get_attestation(&1u64).is_none());
}

#[test]
fn invalid_proof_traps() {
    let f = setup(500_000_000);
    // Flip the mock verifier to reject.
    let mock = crate::test::MockVerifierClient::new(&f.env, &f.verifier);
    mock.set_reject(&true);

    let root = BytesN::from_array(&f.env, &[9u8; 32]);
    let seal = Bytes::from_array(&f.env, &[1, 2, 3, 4]);
    let res = f.client.try_submit_proof(&seal, &root, &10u64, &2u64, &7u32);
    assert!(res.is_err()); // verification trap surfaces as an error
}
