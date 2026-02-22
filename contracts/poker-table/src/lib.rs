#![no_std]
#![allow(deprecated)]

use soroban_sdk::{contract, contractimpl, token, Address, Bytes, BytesN, Env, Symbol, Vec};

mod betting;
mod game;
mod game_hub;
mod pot;
mod test;
mod timeout;
mod types;
mod verifier;

use types::*;

/// TTL for table storage (30 days in ledgers, ~5 seconds per ledger)
const TABLE_TTL_THRESHOLD: u32 = 17_280; // ~1 day — trigger extension when below this
const TABLE_TTL_EXTEND: u32 = 518_400; // ~30 days

#[contract]
pub struct PokerTableContract;

fn load_table(env: &Env, table_id: u32) -> Result<TableState, PokerTableError> {
    let key = DataKey::Table(table_id);
    let table: TableState = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(PokerTableError::TableNotFound)?;
    // Extend TTL on every read to keep active tables alive
    env.storage()
        .persistent()
        .extend_ttl(&key, TABLE_TTL_THRESHOLD, TABLE_TTL_EXTEND);
    Ok(table)
}

fn save_table(env: &Env, table: &TableState) {
    let key = DataKey::Table(table.id);
    env.storage().persistent().set(&key, table);
    env.storage()
        .persistent()
        .extend_ttl(&key, TABLE_TTL_THRESHOLD, TABLE_TTL_EXTEND);
    // Keep instance storage alive too
    env.storage()
        .instance()
        .extend_ttl(TABLE_TTL_THRESHOLD, TABLE_TTL_EXTEND);
}

fn derive_session_id(table_id: u32, hand_number: u32) -> u32 {
    // Deterministic 32-bit hash of (table_id, hand_number).
    let mut x = table_id ^ hand_number.rotate_left(16);
    x = x.wrapping_mul(0x9E37_79B1);
    x ^= x >> 16;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x
}

#[contractimpl]
impl PokerTableContract {
    /// Initialize a new poker table with configuration.
    pub fn create_table(env: Env, admin: Address, config: TableConfig) -> u32 {
        admin.require_auth();

        let table_id = env
            .storage()
            .instance()
            .get::<Symbol, u32>(&Symbol::new(&env, "next_id"))
            .unwrap_or(0);

        let table = TableState {
            id: table_id,
            admin: admin.clone(),
            config: config.clone(),
            phase: GamePhase::Waiting,
            players: Vec::new(&env),
            dealer_seat: 0,
            current_turn: 0,
            pot: 0,
            side_pots: Vec::new(&env),
            deck_root: BytesN::from_array(&env, &[0u8; 32]),
            hand_commitments: Vec::new(&env),
            board_cards: Vec::new(&env),
            dealt_indices: Vec::new(&env),
            hand_number: 0,
            last_action_ledger: env.ledger().sequence(),
            committee: config.committee,
            session_id: 0,
        };

        save_table(&env, &table);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "next_id"), &(table_id + 1));

        env.events()
            .publish((Symbol::new(&env, "table_created"), table_id), admin);

        table_id
    }

    /// Join a table with a buy-in deposit.
    pub fn join_table(
        env: Env,
        table_id: u32,
        player: Address,
        buy_in: i128,
    ) -> Result<u32, PokerTableError> {
        player.require_auth();

        let mut table = load_table(&env, table_id)?;

        if !matches!(table.phase, GamePhase::Waiting) {
            return Err(PokerTableError::TableNotAcceptingPlayers);
        }
        if (table.players.len() as u32) >= table.config.max_players {
            return Err(PokerTableError::TableFull);
        }
        if buy_in < table.config.min_buy_in || buy_in > table.config.max_buy_in {
            return Err(PokerTableError::InvalidBuyIn);
        }

        // Check player not already seated.
        for i in 0..table.players.len() {
            let p = table
                .players
                .get(i)
                .ok_or(PokerTableError::InvalidPlayerIndex)?;
            if p.address == player {
                return Err(PokerTableError::AlreadySeated);
            }
        }

        // Transfer buy-in to contract.
        let token = token::Client::new(&env, &table.config.token);
        token.transfer(&player, &env.current_contract_address(), &buy_in);

        let seat = table.players.len() as u32;
        table.players.push_back(PlayerState {
            address: player.clone(),
            stack: buy_in,
            bet_this_round: 0,
            folded: false,
            all_in: false,
            sitting_out: false,
            seat_index: seat,
        });

        save_table(&env, &table);

        env.events().publish(
            (Symbol::new(&env, "player_joined"), table_id),
            (player, seat),
        );

        Ok(seat)
    }

    /// Leave the table and withdraw remaining stack.
    pub fn leave_table(env: Env, table_id: u32, player: Address) -> Result<i128, PokerTableError> {
        player.require_auth();

        let mut table = load_table(&env, table_id)?;

        // Can only leave during Waiting phase or between hands.
        if !matches!(table.phase, GamePhase::Waiting | GamePhase::Settlement) {
            return Err(PokerTableError::CannotLeaveDuringActiveHand);
        }

        let mut withdrawn: i128 = 0;
        let mut found = false;
        let mut new_players: Vec<PlayerState> = Vec::new(&env);

        for i in 0..table.players.len() {
            let p = table
                .players
                .get(i)
                .ok_or(PokerTableError::InvalidPlayerIndex)?;
            if p.address == player {
                found = true;
                withdrawn = p.stack;
                if withdrawn > 0 {
                    let token = token::Client::new(&env, &table.config.token);
                    token.transfer(&env.current_contract_address(), &player, &withdrawn);
                }
            } else {
                new_players.push_back(p);
            }
        }

        if !found {
            return Err(PokerTableError::PlayerNotAtTable);
        }
        table.players = new_players;

        save_table(&env, &table);

        env.events().publish(
            (Symbol::new(&env, "player_left"), table_id),
            (player, withdrawn),
        );

        Ok(withdrawn)
    }

    /// Start a new hand. Called after enough players are seated.
    pub fn start_hand(env: Env, table_id: u32) -> Result<(), PokerTableError> {
        let mut table = load_table(&env, table_id)?;

        if !matches!(table.phase, GamePhase::Waiting | GamePhase::Settlement) {
            return Err(PokerTableError::HandAlreadyInProgress);
        }
        if table.players.len() < 2 {
            return Err(PokerTableError::NeedAtLeastTwoPlayers);
        }

        game::start_new_hand(&env, &mut table)?;

        // Notify game hub: start_game with first 2 players.
        let p1 = table
            .players
            .get(0)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;
        let p2 = table
            .players
            .get(1)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;

        table.session_id = derive_session_id(table.id, table.hand_number);
        game_hub::notify_start(
            &env,
            &table.config.game_hub,
            &env.current_contract_address(),
            table.session_id,
            &p1.address,
            &p2.address,
            p1.stack,
            p2.stack,
        );

        save_table(&env, &table);

        env.events().publish(
            (Symbol::new(&env, "hand_started"), table_id),
            table.hand_number,
        );

        Ok(())
    }

    /// Committee submits deal commitment and proof.
    pub fn commit_deal(
        env: Env,
        table_id: u32,
        committee: Address,
        deck_root: BytesN<32>,
        hand_commitments: Vec<BytesN<32>>,
        dealt_indices: Vec<u32>,
        proof: Bytes,
        public_inputs: Bytes,
    ) -> Result<(), PokerTableError> {
        committee.require_auth();

        let mut table = load_table(&env, table_id)?;

        if !matches!(table.phase, GamePhase::Dealing) {
            return Err(PokerTableError::NotInDealingPhase);
        }
        if committee != table.committee {
            return Err(PokerTableError::NotAuthorizedCommittee);
        }
        if hand_commitments.len() != table.players.len() {
            return Err(PokerTableError::WrongCommitmentCount);
        }

        // Verify deal proof via ZK verifier contract.
        let verifier_client = verifier::ZkVerifierClient::new(&env, &table.config.verifier);
        if !verifier_client.verify_deal(&proof, &public_inputs, &deck_root, &hand_commitments) {
            return Err(PokerTableError::DealProofVerificationFailed);
        }

        table.deck_root = deck_root;
        table.hand_commitments = hand_commitments;
        table.dealt_indices = dealt_indices;
        table.phase = GamePhase::Preflop;
        table.last_action_ledger = env.ledger().sequence();

        // Set first player to act (left of big blind).
        let num_players = table.players.len() as u32;
        if num_players < 2 {
            return Err(PokerTableError::NeedAtLeastTwoPlayers);
        }
        table.current_turn = (table.dealer_seat + 3) % num_players;

        save_table(&env, &table);

        env.events().publish(
            (Symbol::new(&env, "deal_committed"), table_id),
            table.hand_number,
        );

        Ok(())
    }

    /// Player submits a betting action.
    pub fn player_action(
        env: Env,
        table_id: u32,
        player: Address,
        action: Action,
    ) -> Result<(), PokerTableError> {
        player.require_auth();

        let mut table = load_table(&env, table_id)?;

        if !matches!(
            table.phase,
            GamePhase::Preflop | GamePhase::Flop | GamePhase::Turn | GamePhase::River
        ) {
            return Err(PokerTableError::NotInBettingPhase);
        }

        betting::process_action(&env, &mut table, &player, &action)?;

        save_table(&env, &table);
        Ok(())
    }

    /// Committee reveals board cards (flop/turn/river) with proof.
    pub fn reveal_board(
        env: Env,
        table_id: u32,
        committee: Address,
        cards: Vec<u32>,
        indices: Vec<u32>,
        proof: Bytes,
        public_inputs: Bytes,
    ) -> Result<(), PokerTableError> {
        committee.require_auth();

        let mut table = load_table(&env, table_id)?;

        if committee != table.committee {
            return Err(PokerTableError::NotAuthorizedCommittee);
        }

        let expected_cards: u32 = match table.phase {
            GamePhase::DealingFlop => 3,
            GamePhase::DealingTurn => 1,
            GamePhase::DealingRiver => 1,
            _ => return Err(PokerTableError::NotInRevealPhase),
        };

        if cards.len() != expected_cards || indices.len() != expected_cards {
            return Err(PokerTableError::WrongCardCount);
        }

        // Verify reveal proof via zk-verifier.
        let verifier_client = verifier::ZkVerifierClient::new(&env, &table.config.verifier);
        if !verifier_client.verify_reveal(
            &proof,
            &public_inputs,
            &table.deck_root,
            &cards,
            &indices,
        ) {
            return Err(PokerTableError::RevealProofVerificationFailed);
        }

        // Add revealed cards to board.
        for i in 0..cards.len() {
            table
                .board_cards
                .push_back(cards.get(i).ok_or(PokerTableError::WrongCardCount)?);
            table
                .dealt_indices
                .push_back(indices.get(i).ok_or(PokerTableError::WrongCardCount)?);
        }

        // Transition to next betting phase.
        table.phase = match table.phase {
            GamePhase::DealingFlop => GamePhase::Flop,
            GamePhase::DealingTurn => GamePhase::Turn,
            GamePhase::DealingRiver => GamePhase::River,
            _ => return Err(PokerTableError::NotInRevealPhase),
        };
        table.last_action_ledger = env.ledger().sequence();

        // Reset betting state for new round.
        betting::reset_round(&env, &mut table)?;

        save_table(&env, &table);

        env.events()
            .publish((Symbol::new(&env, "board_revealed"), table_id), cards);

        Ok(())
    }

    /// Submit showdown: reveal hole cards, verify winner, settle.
    pub fn submit_showdown(
        env: Env,
        table_id: u32,
        committee: Address,
        hole_cards: Vec<(u32, u32)>,
        _salts: Vec<(BytesN<32>, BytesN<32>)>,
        proof: Bytes,
        public_inputs: Bytes,
    ) -> Result<(), PokerTableError> {
        committee.require_auth();

        let mut table = load_table(&env, table_id)?;

        if !matches!(table.phase, GamePhase::Showdown) {
            return Err(PokerTableError::NotInShowdownPhase);
        }
        if committee != table.committee {
            return Err(PokerTableError::NotAuthorizedCommittee);
        }

        // Verify showdown proof via zk-verifier.
        let verifier_client = verifier::ZkVerifierClient::new(&env, &table.config.verifier);
        // winner_index = 0 placeholder; the proof itself encodes the winner.
        if !verifier_client.verify_showdown(
            &proof,
            &public_inputs,
            &table.hand_commitments,
            &table.board_cards,
            &0u32,
        ) {
            return Err(PokerTableError::ShowdownProofVerificationFailed);
        }

        // Evaluate hands and determine winner.
        game::settle_showdown(&env, &mut table, &hole_cards)?;

        save_table(&env, &table);
        Ok(())
    }

    /// Claim timeout when opponent or committee is stalling.
    pub fn claim_timeout(env: Env, table_id: u32, claimer: Address) -> Result<(), PokerTableError> {
        claimer.require_auth();

        let mut table = load_table(&env, table_id)?;

        timeout::process_timeout(&env, &mut table, &claimer)?;

        save_table(&env, &table);
        Ok(())
    }

    /// Read current table state (view function).
    pub fn get_table(env: Env, table_id: u32) -> Result<TableState, PokerTableError> {
        load_table(&env, table_id)
    }

    // ========================================================================
    // Admin Functions (Stellar Game Studio pattern)
    // ========================================================================

    /// Get the admin address for a table.
    pub fn get_admin(env: Env, table_id: u32) -> Result<Address, PokerTableError> {
        let table = load_table(&env, table_id)?;
        Ok(table.admin)
    }

    /// Get the Game Hub address for a table.
    pub fn get_hub(env: Env, table_id: u32) -> Result<Address, PokerTableError> {
        let table = load_table(&env, table_id)?;
        Ok(table.config.game_hub)
    }

    /// Update the Game Hub address for a table (admin only).
    pub fn set_hub(
        env: Env,
        table_id: u32,
        new_hub: Address,
    ) -> Result<(), PokerTableError> {
        let mut table = load_table(&env, table_id)?;
        table.admin.require_auth();
        table.config.game_hub = new_hub;
        save_table(&env, &table);
        Ok(())
    }

    /// Upgrade the contract WASM (admin only).
    pub fn upgrade(env: Env, table_id: u32, new_wasm_hash: BytesN<32>) -> Result<(), PokerTableError> {
        let table = load_table(&env, table_id)?;
        table.admin.require_auth();
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }
}
