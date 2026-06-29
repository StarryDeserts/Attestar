#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Bytes,
    BytesN, Env,
};
use risc0_interface::RiscZeroVerifierRouterClient;

#[cfg(test)]
mod test;

#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub admin: Address,
    pub image_id: BytesN<32>,
    pub reserve: Address,
    pub usdc_sac: Address,
    pub verifier_router: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attestation {
    pub root: BytesN<32>,
    pub liabilities: i128,
    pub reserves: i128,
    pub snapshot: u64,
    pub count: u32,
    pub solvent: bool,
}

#[contracttype]
pub enum DataKey {
    Config,
    Attestation(u64),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    Insolvent = 2,
}

#[contract]
pub struct ProofOfReserves;

#[contractimpl]
impl ProofOfReserves {
    pub fn __constructor(
        env: Env,
        admin: Address,
        image_id: BytesN<32>,
        reserve: Address,
        usdc_sac: Address,
        verifier_router: Address,
    ) {
        env.storage().instance().set(
            &DataKey::Config,
            &Config { admin, image_id, reserve, usdc_sac, verifier_router },
        );
    }

    pub fn config(env: Env) -> Option<Config> {
        env.storage().instance().get(&DataKey::Config)
    }

    pub fn submit_proof(
        env: Env,
        seal: Bytes,
        root: BytesN<32>,
        total: u64,
        snapshot: u64,
        count: u32,
    ) -> Result<Attestation, Error> {
        let config: Config =
            env.storage().instance().get(&DataKey::Config).ok_or(Error::NotInitialized)?;
        config.admin.require_auth();

        // Reconstruct the 52-byte journal EXACTLY as the guest committed it:
        // root(32) || total:u64 LE || snapshot:u64 LE || count:u32 LE.
        let mut journal = Bytes::new(&env);
        journal.extend_from_array(&root.to_array());
        journal.extend_from_array(&total.to_le_bytes());
        journal.extend_from_array(&snapshot.to_le_bytes());
        journal.extend_from_array(&count.to_le_bytes());
        let journal_digest = env.crypto().sha256(&journal).to_bytes();

        // Verify the Groth16 receipt through the router (traps if invalid).
        let router = RiscZeroVerifierRouterClient::new(&env, &config.verifier_router);
        router.verify(&seal, &config.image_id, &journal_digest);

        // Bind to LIVE on-chain reserves and enforce solvency.
        let reserves = token::TokenClient::new(&env, &config.usdc_sac).balance(&config.reserve);
        let liabilities = total as i128;
        if reserves < liabilities {
            return Err(Error::Insolvent);
        }

        let attestation = Attestation {
            root: root.clone(),
            liabilities,
            reserves,
            snapshot,
            count,
            solvent: true,
        };
        env.storage().persistent().set(&DataKey::Attestation(snapshot), &attestation);
        // publish() is deprecated in this SDK in favor of #[contractevent]; kept
        // as-is so the source still reproduces the deployed wasm (hash unchanged).
        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("attest"), snapshot),
            (root, liabilities, reserves),
        );
        Ok(attestation)
    }

    pub fn get_attestation(env: Env, snapshot: u64) -> Option<Attestation> {
        env.storage().persistent().get(&DataKey::Attestation(snapshot))
    }
}
