#![no_std]
#![allow(deprecated)]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol, Vec};

/// Committee Registry contract.
///
/// Manages MPC committee membership, staking bonds, and slashing hooks.
/// The committee is responsible for:
/// - Shuffling the deck via MPC
/// - Generating ZK proofs via coNoir
/// - Delivering private cards to players
/// - Responding to reveal requests within timeout
#[contract]
pub struct CommitteeRegistryContract;

#[contracttype]
#[derive(Clone, Debug)]
pub struct CommitteeMember {
    pub address: Address,
    pub stake: i128,
    pub endpoint: soroban_sdk::String, // MPC node endpoint URL
    pub active: bool,
    pub slash_count: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct CommitteeEpoch {
    pub epoch_id: u32,
    pub members: Vec<Address>,
    pub threshold: u32, // Minimum members needed (2 of 3)
    pub start_ledger: u32,
    pub end_ledger: u32, // 0 = no end (current epoch)
}

#[contracttype]
#[derive(Clone, Debug)]
pub enum RegistryKey {
    Admin,
    StakeToken,
    MinStake,
    Member(Address),
    CurrentEpoch,
    Epoch(u32),
    SlashEvent(u32), // slash event counter
}

#[contractimpl]
impl CommitteeRegistryContract {
    /// Initialize the registry.
    pub fn initialize(env: Env, admin: Address, stake_token: Address, min_stake: i128) {
        admin.require_auth();
        assert!(
            !env.storage().instance().has(&RegistryKey::Admin),
            "already initialized"
        );

        env.storage().instance().set(&RegistryKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&RegistryKey::StakeToken, &stake_token);
        env.storage()
            .instance()
            .set(&RegistryKey::MinStake, &min_stake);
    }

    /// Register as a committee member with a stake.
    pub fn register_member(env: Env, member: Address, stake: i128, endpoint: soroban_sdk::String) {
        member.require_auth();

        let min_stake: i128 = env
            .storage()
            .instance()
            .get(&RegistryKey::MinStake)
            .expect("not initialized");
        assert!(stake >= min_stake, "insufficient stake");

        // Transfer stake to contract
        let token_addr: Address = env
            .storage()
            .instance()
            .get(&RegistryKey::StakeToken)
            .unwrap();
        let token = token::Client::new(&env, &token_addr);
        token.transfer(&member, &env.current_contract_address(), &stake);

        let member_state = CommitteeMember {
            address: member.clone(),
            stake,
            endpoint,
            active: true,
            slash_count: 0,
        };

        env.storage()
            .persistent()
            .set(&RegistryKey::Member(member.clone()), &member_state);

        env.events()
            .publish((Symbol::new(&env, "member_registered"),), member);
    }

    /// Withdraw stake and deregister (only when not in active epoch).
    pub fn deregister_member(env: Env, member: Address) -> i128 {
        member.require_auth();

        let mut m: CommitteeMember = env
            .storage()
            .persistent()
            .get(&RegistryKey::Member(member.clone()))
            .expect("not a member");

        // Check not in active epoch
        if let Some(epoch) = Self::get_current_epoch(env.clone()) {
            for i in 0..epoch.members.len() {
                assert!(
                    epoch.members.get(i).unwrap() != member,
                    "cannot deregister during active epoch"
                );
            }
        }

        let stake = m.stake;
        m.active = false;
        m.stake = 0;

        // Return stake
        let token_addr: Address = env
            .storage()
            .instance()
            .get(&RegistryKey::StakeToken)
            .unwrap();
        let token = token::Client::new(&env, &token_addr);
        token.transfer(&env.current_contract_address(), &member, &stake);

        env.storage()
            .persistent()
            .set(&RegistryKey::Member(member.clone()), &m);

        env.events()
            .publish((Symbol::new(&env, "member_deregistered"),), member);

        stake
    }

    /// Admin creates a new committee epoch with selected members.
    pub fn create_epoch(env: Env, admin: Address, members: Vec<Address>, threshold: u32) -> u32 {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&RegistryKey::Admin)
            .expect("not initialized");
        assert!(admin == stored_admin, "not admin");
        assert!(
            members.len() >= threshold,
            "not enough members for threshold"
        );

        // Verify all members are registered and active
        for i in 0..members.len() {
            let addr = members.get(i).unwrap();
            let m: CommitteeMember = env
                .storage()
                .persistent()
                .get(&RegistryKey::Member(addr.clone()))
                .expect("member not registered");
            assert!(m.active, "member not active");
        }

        // Close previous epoch
        let prev_epoch_id: u32 = env
            .storage()
            .instance()
            .get(&RegistryKey::CurrentEpoch)
            .unwrap_or(0);

        if prev_epoch_id > 0 {
            let mut prev: CommitteeEpoch = env
                .storage()
                .persistent()
                .get(&RegistryKey::Epoch(prev_epoch_id))
                .unwrap();
            prev.end_ledger = env.ledger().sequence();
            env.storage()
                .persistent()
                .set(&RegistryKey::Epoch(prev_epoch_id), &prev);
        }

        let epoch_id = prev_epoch_id + 1;
        let epoch = CommitteeEpoch {
            epoch_id,
            members: members.clone(),
            threshold,
            start_ledger: env.ledger().sequence(),
            end_ledger: 0,
        };

        env.storage()
            .persistent()
            .set(&RegistryKey::Epoch(epoch_id), &epoch);
        env.storage()
            .instance()
            .set(&RegistryKey::CurrentEpoch, &epoch_id);

        env.events()
            .publish((Symbol::new(&env, "epoch_created"), epoch_id), members);

        epoch_id
    }

    /// Trigger a slashing event against a committee member.
    /// Called by PokerTable contract when committee fails to act within timeout.
    pub fn report_slash(env: Env, reporter: Address, member: Address, reason: Symbol) {
        reporter.require_auth();

        // In production, verify reporter is an authorized PokerTable contract
        // For v1, any address can report (admin will adjudicate)

        let mut m: CommitteeMember = env
            .storage()
            .persistent()
            .get(&RegistryKey::Member(member.clone()))
            .expect("not a member");

        m.slash_count += 1;

        // Emit slash event for off-chain monitoring
        env.events().publish(
            (Symbol::new(&env, "slash_reported"), m.slash_count),
            (member.clone(), reason),
        );

        // If slash count exceeds threshold, deactivate and slash stake
        if m.slash_count >= 3 {
            let slashed = m.stake / 2; // Slash 50%
            m.stake -= slashed;
            m.active = false;
            // Slashed funds stay in contract (can be distributed to affected players)
        }

        env.storage()
            .persistent()
            .set(&RegistryKey::Member(member), &m);
    }

    /// View the current epoch.
    pub fn get_current_epoch(env: Env) -> Option<CommitteeEpoch> {
        let epoch_id: u32 = env
            .storage()
            .instance()
            .get(&RegistryKey::CurrentEpoch)
            .unwrap_or(0);

        if epoch_id == 0 {
            return None;
        }

        env.storage()
            .persistent()
            .get(&RegistryKey::Epoch(epoch_id))
    }

    /// View a member's state.
    pub fn get_member(env: Env, member: Address) -> CommitteeMember {
        env.storage()
            .persistent()
            .get(&RegistryKey::Member(member))
            .expect("not a member")
    }
}
