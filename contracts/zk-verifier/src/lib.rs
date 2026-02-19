#![no_std]
#![allow(deprecated)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env, Symbol, Vec,
};
use ultrahonk_soroban_verifier::{UltraHonkVerifier, PROOF_BYTES};

/// ZK Verifier contract for Stellar Poker.
///
/// Uses UltraHonk proof verification via Soroban's native BN254 host functions
/// (Protocol 25 / X-Ray). Each circuit type has its own verification key (VK)
/// stored on-chain. Proofs are verified against their circuit's VK and the
/// provided public inputs.
#[contract]
pub struct ZkVerifierContract;

#[contracterror]
#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VerifierError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NotAdmin = 3,
    NoVkForCircuit = 4,
    VkParseError = 5,
    ProofSizeError = 6,
    VerificationFailed = 7,
}

#[contracttype]
#[derive(Clone)]
pub enum CircuitType {
    DealValid,
    RevealBoardValid,
    ShowdownValid,
}

#[contracttype]
#[derive(Clone)]
pub enum StorageKey {
    Admin,
    Vk(CircuitType),
    ProofVerified(BytesN<32>),
}

#[contractimpl]
impl ZkVerifierContract {
    /// Initialize the verifier with an admin.
    pub fn initialize(env: Env, admin: Address) -> Result<(), VerifierError> {
        if env.storage().instance().has(&StorageKey::Admin) {
            return Err(VerifierError::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&StorageKey::Admin, &admin);
        Ok(())
    }

    /// Store a verification key for a circuit type.
    /// Called once per circuit during deployment.
    pub fn set_verification_key(
        env: Env,
        admin: Address,
        circuit: CircuitType,
        vk_data: Bytes,
    ) -> Result<(), VerifierError> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&StorageKey::Admin)
            .ok_or(VerifierError::NotInitialized)?;
        if admin != stored_admin {
            return Err(VerifierError::NotAdmin);
        }

        // Validate the VK can be parsed before storing
        UltraHonkVerifier::new(&env, &vk_data).map_err(|_| VerifierError::VkParseError)?;

        env.storage()
            .persistent()
            .set(&StorageKey::Vk(circuit.clone()), &vk_data);

        env.events()
            .publish((Symbol::new(&env, "vk_set"),), circuit);
        Ok(())
    }

    /// Verify an UltraHonk proof for a given circuit type.
    ///
    /// 1. Loads the VK for the circuit type
    /// 2. Validates proof size (14,592 bytes = 456 fields * 32)
    /// 3. Runs full UltraHonk verification (sumcheck + shplonk pairing)
    /// 4. Stores proof hash for auditability
    pub fn verify_proof(
        env: Env,
        circuit: CircuitType,
        proof: Bytes,
        public_inputs: Bytes,
    ) -> Result<bool, VerifierError> {
        // Check proof size
        if proof.len() as usize != PROOF_BYTES {
            return Err(VerifierError::ProofSizeError);
        }

        // Load VK for this circuit
        let vk_bytes: Bytes = env
            .storage()
            .persistent()
            .get(&StorageKey::Vk(circuit))
            .ok_or(VerifierError::NoVkForCircuit)?;

        // Parse VK and create verifier
        let verifier =
            UltraHonkVerifier::new(&env, &vk_bytes).map_err(|_| VerifierError::VkParseError)?;

        // Run full UltraHonk verification
        verifier
            .verify(&proof, &public_inputs)
            .map_err(|_| VerifierError::VerificationFailed)?;

        // Store proof hash for auditability
        let proof_hash = env.crypto().keccak256(&proof);
        env.storage()
            .persistent()
            .set(&StorageKey::ProofVerified(proof_hash.clone().into()), &true);

        env.events()
            .publish((Symbol::new(&env, "proof_verified"),), proof_hash);

        Ok(true)
    }

    /// Check if a proof was previously verified.
    pub fn is_proof_verified(env: Env, proof_hash: BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .get(&StorageKey::ProofVerified(proof_hash))
            .unwrap_or(false)
    }

    /// Verify a deal proof. Validates format and delegates to verify_proof.
    pub fn verify_deal(
        env: Env,
        proof: Bytes,
        public_inputs: Bytes,
        _deck_root: BytesN<32>,
        _hand_commitments: Vec<BytesN<32>>,
    ) -> Result<bool, VerifierError> {
        Self::verify_proof(env, CircuitType::DealValid, proof, public_inputs)
    }

    /// Verify a board reveal proof.
    pub fn verify_reveal(
        env: Env,
        proof: Bytes,
        public_inputs: Bytes,
        _deck_root: BytesN<32>,
        _revealed_cards: Vec<u32>,
        _revealed_indices: Vec<u32>,
    ) -> Result<bool, VerifierError> {
        Self::verify_proof(env, CircuitType::RevealBoardValid, proof, public_inputs)
    }

    /// Verify a showdown proof.
    pub fn verify_showdown(
        env: Env,
        proof: Bytes,
        public_inputs: Bytes,
        _hand_commitments: Vec<BytesN<32>>,
        _board_cards: Vec<u32>,
        _winner_index: u32,
    ) -> Result<bool, VerifierError> {
        Self::verify_proof(env, CircuitType::ShowdownValid, proof, public_inputs)
    }
}
