#![no_std]
#![allow(deprecated)]

use soroban_sdk::{
    contract, contractimpl, token, Address, BytesN, Env, Symbol, Vec,
};

mod betting;
mod game;
mod game_hub;
mod pot;
mod test;
mod timeout;
mod types;
mod verifier;

use types::*;

#[contract]
pub struct PokerTableContract;

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

        env.storage()
            .persistent()
            .set(&DataKey::Table(table_id), &table);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "next_id"), &(table_id + 1));

        env.events().publish(
            (Symbol::new(&env, "table_created"), table_id),
            admin,
        );

        table_id
    }

    /// Join a table with a buy-in deposit.
    pub fn join_table(env: Env, table_id: u32, player: Address, buy_in: i128) -> u32 {
        player.require_auth();

        let mut table: TableState = env
            .storage()
            .persistent()
            .get(&DataKey::Table(table_id))
            .expect("table not found");

        assert!(
            matches!(table.phase, GamePhase::Waiting),
            "table not accepting players"
        );
        assert!(
            (table.players.len() as u32) < table.config.max_players,
            "table full"
        );
        assert!(
            buy_in >= table.config.min_buy_in && buy_in <= table.config.max_buy_in,
            "invalid buy-in"
        );

        // Check player not already seated
        for i in 0..table.players.len() {
            let p = table.players.get(i).unwrap();
            assert!(p.address != player, "already seated");
        }

        // Transfer buy-in to contract
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

        env.storage()
            .persistent()
            .set(&DataKey::Table(table_id), &table);

        env.events().publish(
            (Symbol::new(&env, "player_joined"), table_id),
            (player, seat),
        );

        seat
    }

    /// Leave the table and withdraw remaining stack.
    pub fn leave_table(env: Env, table_id: u32, player: Address) -> i128 {
        player.require_auth();

        let mut table: TableState = env
            .storage()
            .persistent()
            .get(&DataKey::Table(table_id))
            .expect("table not found");

        // Can only leave during Waiting phase or between hands
        assert!(
            matches!(table.phase, GamePhase::Waiting | GamePhase::Settlement),
            "cannot leave during active hand"
        );

        let mut withdrawn: i128 = 0;
        let mut found = false;
        let mut new_players: Vec<PlayerState> = Vec::new(&env);

        for i in 0..table.players.len() {
            let p = table.players.get(i).unwrap();
            if p.address == player {
                found = true;
                withdrawn = p.stack;
                // Transfer back to player
                if withdrawn > 0 {
                    let token = token::Client::new(&env, &table.config.token);
                    token.transfer(&env.current_contract_address(), &player, &withdrawn);
                }
            } else {
                new_players.push_back(p);
            }
        }

        assert!(found, "player not at table");
        table.players = new_players;

        env.storage()
            .persistent()
            .set(&DataKey::Table(table_id), &table);

        env.events().publish(
            (Symbol::new(&env, "player_left"), table_id),
            (player, withdrawn),
        );

        withdrawn
    }

    /// Start a new hand. Called after enough players are seated.
    pub fn start_hand(env: Env, table_id: u32) {
        let mut table: TableState = env
            .storage()
            .persistent()
            .get(&DataKey::Table(table_id))
            .expect("table not found");

        assert!(
            matches!(table.phase, GamePhase::Waiting | GamePhase::Settlement),
            "hand already in progress"
        );
        assert!(table.players.len() >= 2, "need at least 2 players");

        game::start_new_hand(&env, &mut table);

        // Notify game hub: start_game with first 2 players
        let p1 = table.players.get(0).unwrap();
        let p2 = table.players.get(1).unwrap();
        table.session_id = table.hand_number; // Use hand number as session ID
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

        env.storage()
            .persistent()
            .set(&DataKey::Table(table_id), &table);

        env.events().publish(
            (Symbol::new(&env, "hand_started"), table_id),
            table.hand_number,
        );
    }

    /// Committee submits deal commitment and proof.
    pub fn commit_deal(
        env: Env,
        table_id: u32,
        committee: Address,
        deck_root: BytesN<32>,
        hand_commitments: Vec<BytesN<32>>,
        dealt_indices: Vec<u32>,
        proof: soroban_sdk::Bytes,
        public_inputs: soroban_sdk::Bytes,
    ) {
        committee.require_auth();

        let mut table: TableState = env
            .storage()
            .persistent()
            .get(&DataKey::Table(table_id))
            .expect("table not found");

        assert!(
            matches!(table.phase, GamePhase::Dealing),
            "not in dealing phase"
        );
        assert!(committee == table.committee, "not authorized committee");
        assert!(
            hand_commitments.len() == table.players.len(),
            "wrong number of commitments"
        );

        // Verify deal proof via ZK verifier contract
        let verifier_client =
            verifier::ZkVerifierClient::new(&env, &table.config.verifier);
        assert!(
            verifier_client.verify_deal(&proof, &public_inputs, &deck_root, &hand_commitments),
            "deal proof verification failed"
        );

        table.deck_root = deck_root;
        table.hand_commitments = hand_commitments;
        table.dealt_indices = dealt_indices;
        table.phase = GamePhase::Preflop;
        table.last_action_ledger = env.ledger().sequence();

        // Set first player to act (left of big blind)
        let num_players = table.players.len() as u32;
        table.current_turn = (table.dealer_seat + 3) % num_players;

        env.storage()
            .persistent()
            .set(&DataKey::Table(table_id), &table);

        env.events().publish(
            (Symbol::new(&env, "deal_committed"), table_id),
            table.hand_number,
        );
    }

    /// Player submits a betting action.
    pub fn player_action(env: Env, table_id: u32, player: Address, action: Action) {
        player.require_auth();

        let mut table: TableState = env
            .storage()
            .persistent()
            .get(&DataKey::Table(table_id))
            .expect("table not found");

        assert!(
            matches!(
                table.phase,
                GamePhase::Preflop
                    | GamePhase::Flop
                    | GamePhase::Turn
                    | GamePhase::River
            ),
            "not in betting phase"
        );

        betting::process_action(&env, &mut table, &player, &action);

        env.storage()
            .persistent()
            .set(&DataKey::Table(table_id), &table);
    }

    /// Committee reveals board cards (flop/turn/river) with proof.
    pub fn reveal_board(
        env: Env,
        table_id: u32,
        committee: Address,
        cards: Vec<u32>,
        indices: Vec<u32>,
        proof: soroban_sdk::Bytes,
        public_inputs: soroban_sdk::Bytes,
    ) {
        committee.require_auth();

        let mut table: TableState = env
            .storage()
            .persistent()
            .get(&DataKey::Table(table_id))
            .expect("table not found");

        assert!(committee == table.committee, "not authorized committee");

        // Validate correct phase and card count
        let expected_cards: u32 = match table.phase {
            GamePhase::DealingFlop => 3,
            GamePhase::DealingTurn => 1,
            GamePhase::DealingRiver => 1,
            _ => panic!("not in reveal phase"),
        };
        assert!(cards.len() == expected_cards, "wrong number of cards");

        // Verify reveal proof via zk-verifier
        let verifier_client =
            verifier::ZkVerifierClient::new(&env, &table.config.verifier);
        assert!(
            verifier_client.verify_reveal(
                &proof,
                &public_inputs,
                &table.deck_root,
                &cards,
                &indices,
            ),
            "reveal proof verification failed"
        );

        // Add revealed cards to board
        for i in 0..cards.len() {
            table.board_cards.push_back(cards.get(i).unwrap());
            table.dealt_indices.push_back(indices.get(i).unwrap());
        }

        // Transition to next betting phase
        table.phase = match table.phase {
            GamePhase::DealingFlop => GamePhase::Flop,
            GamePhase::DealingTurn => GamePhase::Turn,
            GamePhase::DealingRiver => GamePhase::River,
            _ => unreachable!(),
        };
        table.last_action_ledger = env.ledger().sequence();

        // Reset betting state for new round
        betting::reset_round(&env, &mut table);

        env.storage()
            .persistent()
            .set(&DataKey::Table(table_id), &table);

        env.events().publish(
            (Symbol::new(&env, "board_revealed"), table_id),
            cards,
        );
    }

    /// Submit showdown: reveal hole cards, verify winner, settle.
    pub fn submit_showdown(
        env: Env,
        table_id: u32,
        committee: Address,
        hole_cards: Vec<(u32, u32)>,
        salts: Vec<(BytesN<32>, BytesN<32>)>,
        proof: soroban_sdk::Bytes,
        public_inputs: soroban_sdk::Bytes,
    ) {
        committee.require_auth();

        let mut table: TableState = env
            .storage()
            .persistent()
            .get(&DataKey::Table(table_id))
            .expect("table not found");

        assert!(
            matches!(table.phase, GamePhase::Showdown),
            "not in showdown phase"
        );
        assert!(committee == table.committee, "not authorized committee");

        // Verify showdown proof via zk-verifier
        let verifier_client =
            verifier::ZkVerifierClient::new(&env, &table.config.verifier);
        // winner_index = 0 placeholder; the proof itself encodes the winner
        assert!(
            verifier_client.verify_showdown(
                &proof,
                &public_inputs,
                &table.hand_commitments,
                &table.board_cards,
                &0u32,
            ),
            "showdown proof verification failed"
        );

        let _ = salts; // salts validated inside the ZK proof

        // Evaluate hands and determine winner
        game::settle_showdown(&env, &mut table, &hole_cards);

        env.storage()
            .persistent()
            .set(&DataKey::Table(table_id), &table);
    }

    /// Claim timeout when opponent or committee is stalling.
    pub fn claim_timeout(env: Env, table_id: u32, claimer: Address) {
        claimer.require_auth();

        let mut table: TableState = env
            .storage()
            .persistent()
            .get(&DataKey::Table(table_id))
            .expect("table not found");

        timeout::process_timeout(&env, &mut table, &claimer);

        env.storage()
            .persistent()
            .set(&DataKey::Table(table_id), &table);
    }

    /// Read current table state (view function).
    pub fn get_table(env: Env, table_id: u32) -> TableState {
        env.storage()
            .persistent()
            .get(&DataKey::Table(table_id))
            .expect("table not found")
    }
}
