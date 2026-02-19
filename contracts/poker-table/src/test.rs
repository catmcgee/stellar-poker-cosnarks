#[cfg(test)]
mod test {
    use crate::types::*;
    use crate::{PokerTableContract, PokerTableContractClient};
    use soroban_sdk::{
        testutils::{Address as _, Ledger as _},
        token::{StellarAssetClient, TokenClient},
        Address, BytesN, Env, Vec,
    };

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    /// Deploy a Stellar Asset Contract and mint tokens to the given addresses.
    fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        (
            TokenClient::new(env, &sac.address()),
            StellarAssetClient::new(env, &sac.address()),
        )
    }

    /// Build a standard table config for tests.
    fn default_config(
        env: &Env,
        token: &Address,
        committee: &Address,
        verifier: &Address,
    ) -> TableConfig {
        // Register a mock game hub contract
        let game_hub = env.register(crate::game_hub::GameHubContract, ());
        TableConfig {
            token: token.clone(),
            min_buy_in: 100,
            max_buy_in: 1000,
            small_blind: 5,
            big_blind: 10,
            max_players: 6,
            timeout_ledgers: 100,
            committee: committee.clone(),
            verifier: verifier.clone(),
            game_hub,
        }
    }

    struct TestSetup<'a> {
        env: Env,
        client: PokerTableContractClient<'a>,
        token: TokenClient<'a>,
        token_admin_client: StellarAssetClient<'a>,
        admin: Address,
        committee: Address,
        verifier: Address,
    }

    /// Spin up an environment with a poker-table contract and a token contract.
    fn setup() -> TestSetup<'static> {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(PokerTableContract, ());
        let client = PokerTableContractClient::new(&env, &contract_id);

        let token_admin = Address::generate(&env);
        let (token, token_admin_client) = create_token(&env, &token_admin);

        let admin = Address::generate(&env);
        let committee = Address::generate(&env);
        let verifier = env.register(crate::verifier::ZkVerifierContract, ());

        TestSetup {
            env,
            client,
            token,
            token_admin_client,
            admin,
            committee,
            verifier,
        }
    }

    /// Create a table using the default config and return its id.
    fn create_default_table(s: &TestSetup) -> u32 {
        let config = default_config(&s.env, &s.token.address, &s.committee, &s.verifier);
        s.client.create_table(&s.admin, &config)
    }

    /// Mint tokens, join the table, and return the assigned seat index.
    fn join_player(s: &TestSetup, table_id: u32, player: &Address, buy_in: i128) -> u32 {
        s.token_admin_client.mint(player, &buy_in);
        s.client.join_table(&table_id, player, &buy_in)
    }

    /// Helper to move a table from Dealing -> Preflop by committing a mock deal.
    fn commit_mock_deal(s: &TestSetup, table_id: u32, num_players: u32) {
        let deck_root = BytesN::from_array(&s.env, &[1u8; 32]);
        let mut commitments: Vec<BytesN<32>> = Vec::new(&s.env);
        for _ in 0..num_players {
            commitments.push_back(BytesN::from_array(&s.env, &[2u8; 32]));
        }
        let mut dealt_indices: Vec<u32> = Vec::new(&s.env);
        for i in 0..(num_players * 2) {
            dealt_indices.push_back(i);
        }
        let proof = soroban_sdk::Bytes::new(&s.env);
        let public_inputs = soroban_sdk::Bytes::new(&s.env);

        s.client.commit_deal(
            &table_id,
            &s.committee,
            &deck_root,
            &commitments,
            &dealt_indices,
            &proof,
            &public_inputs,
        );
    }

    // ---------------------------------------------------------------------------
    // 1. Create table
    // ---------------------------------------------------------------------------

    #[test]
    fn test_create_table() {
        let s = setup();
        let table_id = create_default_table(&s);
        assert_eq!(table_id, 0);

        let table = s.client.get_table(&table_id);
        assert_eq!(table.id, 0);
        assert_eq!(table.admin, s.admin);
        assert_eq!(table.config.min_buy_in, 100);
        assert_eq!(table.config.max_buy_in, 1000);
        assert_eq!(table.config.small_blind, 5);
        assert_eq!(table.config.big_blind, 10);
        assert_eq!(table.config.max_players, 6);
        assert_eq!(table.phase, GamePhase::Waiting);
        assert_eq!(table.players.len(), 0);
        assert_eq!(table.pot, 0);
    }

    #[test]
    fn test_create_multiple_tables() {
        let s = setup();
        let id0 = create_default_table(&s);
        let id1 = create_default_table(&s);
        let id2 = create_default_table(&s);
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    // ---------------------------------------------------------------------------
    // 2. Join table with buy-in
    // ---------------------------------------------------------------------------

    #[test]
    fn test_join_table() {
        let s = setup();
        let table_id = create_default_table(&s);

        let player = Address::generate(&s.env);
        let seat = join_player(&s, table_id, &player, 500);
        assert_eq!(seat, 0);

        let table = s.client.get_table(&table_id);
        assert_eq!(table.players.len(), 1);

        let p = table.players.get(0).unwrap();
        assert_eq!(p.address, player);
        assert_eq!(p.stack, 500);
        assert_eq!(p.seat_index, 0);
        assert!(!p.folded);
        assert!(!p.all_in);

        // Token should have been transferred to the contract
        assert_eq!(s.token.balance(&player), 0);
        assert_eq!(s.token.balance(&s.client.address), 500);
    }

    #[test]
    fn test_join_multiple_players() {
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        let p3 = Address::generate(&s.env);

        assert_eq!(join_player(&s, table_id, &p1, 200), 0);
        assert_eq!(join_player(&s, table_id, &p2, 300), 1);
        assert_eq!(join_player(&s, table_id, &p3, 500), 2);

        let table = s.client.get_table(&table_id);
        assert_eq!(table.players.len(), 3);
        assert_eq!(s.token.balance(&s.client.address), 1000);
    }

    #[test]
    #[should_panic(expected = "invalid buy-in")]
    fn test_join_table_buy_in_too_low() {
        let s = setup();
        let table_id = create_default_table(&s);
        let player = Address::generate(&s.env);
        join_player(&s, table_id, &player, 50); // min is 100
    }

    #[test]
    #[should_panic(expected = "invalid buy-in")]
    fn test_join_table_buy_in_too_high() {
        let s = setup();
        let table_id = create_default_table(&s);
        let player = Address::generate(&s.env);
        join_player(&s, table_id, &player, 2000); // max is 1000
    }

    #[test]
    #[should_panic(expected = "already seated")]
    fn test_join_table_already_seated() {
        let s = setup();
        let table_id = create_default_table(&s);
        let player = Address::generate(&s.env);
        join_player(&s, table_id, &player, 500);
        // Mint more so the transfer wouldn't fail first
        s.token_admin_client.mint(&player, &500);
        s.client.join_table(&table_id, &player, &500);
    }

    // ---------------------------------------------------------------------------
    // 3. Start hand
    // ---------------------------------------------------------------------------

    #[test]
    fn test_start_hand() {
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 500);
        join_player(&s, table_id, &p2, 500);

        s.client.start_hand(&table_id);

        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::Dealing);
        assert_eq!(table.hand_number, 1);

        // Blinds should be posted (dealer rotated to seat 1, sb=seat 0, bb=seat 1
        // for 2 players: dealer_seat = (0+1)%2 = 1, sb = (1+1)%2 = 0, bb = (1+2)%2 = 1)
        let sb_player = table.players.get(0).unwrap();
        let bb_player = table.players.get(1).unwrap();
        assert_eq!(sb_player.bet_this_round, 5);  // small blind
        assert_eq!(sb_player.stack, 495);
        assert_eq!(bb_player.bet_this_round, 10); // big blind
        assert_eq!(bb_player.stack, 490);
        assert_eq!(table.pot, 15); // 5 + 10
    }

    #[test]
    #[should_panic(expected = "need at least 2 players")]
    fn test_start_hand_not_enough_players() {
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 500);

        s.client.start_hand(&table_id);
    }

    // ---------------------------------------------------------------------------
    // 4. Full betting round (check, bet, call, fold)
    // ---------------------------------------------------------------------------

    /// Helper: set up a 3-player table and get it into Preflop betting phase.
    fn setup_preflop_3p() -> (TestSetup<'static>, u32, Address, Address, Address) {
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        let p3 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 500);
        join_player(&s, table_id, &p2, 500);
        join_player(&s, table_id, &p3, 500);

        // Start hand -> Dealing
        s.client.start_hand(&table_id);

        // Commit deal -> Preflop
        commit_mock_deal(&s, table_id, 3);

        (s, table_id, p1, p2, p3)
    }

    #[test]
    fn test_commit_deal_transitions_to_preflop() {
        let (s, table_id, _, _, _) = setup_preflop_3p();
        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::Preflop);
    }

    #[test]
    fn test_player_fold() {
        let (s, table_id, _p1, _p2, _p3) = setup_preflop_3p();

        // After start_hand with 3 players:
        //   dealer_seat = 1, sb = seat 2, bb = seat 0
        //   current_turn set by commit_deal = (1+3)%3 = 1
        //   So seat 1 acts first.
        let table = s.client.get_table(&table_id);
        let current = table.current_turn;
        let acting_player = table.players.get(current).unwrap();

        s.client
            .player_action(&table_id, &acting_player.address, &Action::Fold);

        let table = s.client.get_table(&table_id);
        let folded_player = table.players.get(current).unwrap();
        assert!(folded_player.folded);
    }

    #[test]
    fn test_player_call() {
        let (s, table_id, _p1, _p2, _p3) = setup_preflop_3p();

        let table = s.client.get_table(&table_id);
        let current = table.current_turn;
        let acting_player = table.players.get(current).unwrap();
        let stack_before = acting_player.stack;
        let pot_before = table.pot;

        // The current bet is the big blind (10). The acting player's bet_this_round
        // could be 0 or some blind amount depending on seat. We need to call.
        let to_call = {
            let mut max_bet: i128 = 0;
            for i in 0..table.players.len() {
                let p = table.players.get(i).unwrap();
                if p.bet_this_round > max_bet {
                    max_bet = p.bet_this_round;
                }
            }
            max_bet - acting_player.bet_this_round
        };

        s.client
            .player_action(&table_id, &acting_player.address, &Action::Call);

        let table_after = s.client.get_table(&table_id);
        let player_after = table_after.players.get(current).unwrap();
        assert_eq!(player_after.stack, stack_before - to_call);
        assert_eq!(table_after.pot, pot_before + to_call);
    }

    #[test]
    fn test_player_bet() {
        // To test Bet, we need a situation where current_bet == 0 (post-flop).
        // In the current contract, the preflop round ends as soon as all active
        // players have matching bets (SB calls -> all at BB level -> round over).
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 500);
        join_player(&s, table_id, &p2, 500);

        s.client.start_hand(&table_id);
        commit_mock_deal(&s, table_id, 2);

        // 2-player: dealer_seat = 1, sb = seat 0, bb = seat 1
        // commit_deal sets current_turn = (1+3)%2 = 0 (the SB)
        let table = s.client.get_table(&table_id);
        let current = table.current_turn;
        let acting = table.players.get(current).unwrap();

        // SB calls the big blind. Once bets match, round ends automatically.
        s.client
            .player_action(&table_id, &acting.address, &Action::Call);

        // Round completes -> DealingFlop
        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::DealingFlop);

        // Reveal flop
        let flop_cards: Vec<u32> = Vec::from_array(&s.env, [10, 20, 30]);
        let flop_indices: Vec<u32> = Vec::from_array(&s.env, [4, 5, 6]);
        let proof = soroban_sdk::Bytes::new(&s.env);
        let pub_inputs = soroban_sdk::Bytes::new(&s.env);
        s.client.reveal_board(
            &table_id,
            &s.committee,
            &flop_cards,
            &flop_indices,
            &proof,
            &pub_inputs,
        );

        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::Flop);

        // Now current_bet is 0 after reset_round. First active player can Bet.
        let current = table.current_turn;
        let acting = table.players.get(current).unwrap();
        let stack_before = acting.stack;
        let pot_before = table.pot;
        let bet_amount: i128 = 20;

        s.client.player_action(
            &table_id,
            &acting.address,
            &Action::Bet(bet_amount),
        );

        let table = s.client.get_table(&table_id);
        let player_after = table.players.get(current).unwrap();
        assert_eq!(player_after.stack, stack_before - bet_amount);
        assert_eq!(player_after.bet_this_round, bet_amount);
        assert_eq!(table.pot, pot_before + bet_amount);
    }

    #[test]
    fn test_fold_wins_pot() {
        // 2-player: one folds, the other wins the pot.
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 500);
        join_player(&s, table_id, &p2, 500);

        s.client.start_hand(&table_id);
        commit_mock_deal(&s, table_id, 2);

        let table = s.client.get_table(&table_id);
        let pot = table.pot; // Should be 15 (sb 5 + bb 10)
        assert_eq!(pot, 15);

        let current = table.current_turn;
        let folder = table.players.get(current).unwrap();
        let other_seat = if current == 0 { 1u32 } else { 0u32 };
        let winner_before = table.players.get(other_seat).unwrap();
        let winner_stack_before = winner_before.stack;

        // Player folds
        s.client
            .player_action(&table_id, &folder.address, &Action::Fold);

        // Table should be in Settlement with pot awarded to remaining player
        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::Settlement);
        assert_eq!(table.pot, 0);

        let winner_after = table.players.get(other_seat).unwrap();
        assert_eq!(winner_after.stack, winner_stack_before + pot);
    }

    #[test]
    fn test_full_preflop_round_call_call() {
        // 3-player hand: two players call the big blind, round completes.
        // Note: due to is_round_complete logic, the round ends as soon as all
        // active players have matching bets. The BB does not get an extra action.
        let (s, table_id, _p1, _p2, _p3) = setup_preflop_3p();

        // Preflop: dealer=1, sb=2, bb=0, first_to_act = (1+3)%3 = 1
        // Blinds: sb(seat 2) = 5, bb(seat 0) = 10
        let table = s.client.get_table(&table_id);
        let pot_start = table.pot; // 15 (5 + 10)
        assert_eq!(pot_start, 15);

        // Seat 1 (first_to_act) calls the BB (bet 10)
        let turn1 = table.current_turn;
        assert_eq!(turn1, 1);
        let player1 = table.players.get(turn1).unwrap();
        s.client
            .player_action(&table_id, &player1.address, &Action::Call);

        // Seat 2 (SB, bet was 5) calls (adds 5 to match BB at 10)
        let table = s.client.get_table(&table_id);
        let turn2 = table.current_turn;
        assert_eq!(turn2, 2);
        let player2 = table.players.get(turn2).unwrap();
        s.client
            .player_action(&table_id, &player2.address, &Action::Call);

        // All bets now match at 10 -> round ends automatically -> DealingFlop
        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::DealingFlop);
        // Pot: 15 (blinds) + 10 (seat 1 call) + 5 (seat 2 call) = 30
        assert_eq!(table.pot, 30);
    }

    #[test]
    fn test_raise_and_call_sequence() {
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 500);
        join_player(&s, table_id, &p2, 500);

        s.client.start_hand(&table_id);
        commit_mock_deal(&s, table_id, 2);

        // 2 players: dealer=1, sb=0, bb=1
        // current_turn = (1+3)%2 = 0
        let table = s.client.get_table(&table_id);
        let current = table.current_turn;
        let raiser = table.players.get(current).unwrap();

        // Player raises by 20 on top of calling the big blind
        s.client.player_action(
            &table_id,
            &raiser.address,
            &Action::Raise(20),
        );

        // Other player calls the raise
        let table = s.client.get_table(&table_id);
        let current = table.current_turn;
        let caller = table.players.get(current).unwrap();
        s.client
            .player_action(&table_id, &caller.address, &Action::Call);

        // Round should advance to DealingFlop
        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::DealingFlop);
    }

    #[test]
    fn test_all_in_action() {
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 200);
        join_player(&s, table_id, &p2, 200);

        s.client.start_hand(&table_id);
        commit_mock_deal(&s, table_id, 2);

        let table = s.client.get_table(&table_id);
        let current = table.current_turn;
        let player = table.players.get(current).unwrap();

        // Go all-in
        s.client
            .player_action(&table_id, &player.address, &Action::AllIn);

        let table = s.client.get_table(&table_id);
        let p = table.players.get(current).unwrap();
        assert!(p.all_in);
        assert_eq!(p.stack, 0);
    }

    // ---------------------------------------------------------------------------
    // 5. Leave table and withdraw
    // ---------------------------------------------------------------------------

    #[test]
    fn test_leave_table_waiting_phase() {
        let s = setup();
        let table_id = create_default_table(&s);

        let player = Address::generate(&s.env);
        join_player(&s, table_id, &player, 500);

        // Verify player has 0 tokens (transferred to contract)
        assert_eq!(s.token.balance(&player), 0);

        // Leave table (in Waiting phase)
        let withdrawn = s.client.leave_table(&table_id, &player);
        assert_eq!(withdrawn, 500);

        // Tokens returned to player
        assert_eq!(s.token.balance(&player), 500);
        assert_eq!(s.token.balance(&s.client.address), 0);

        // Player removed from table
        let table = s.client.get_table(&table_id);
        assert_eq!(table.players.len(), 0);
    }

    #[test]
    fn test_leave_table_settlement_phase() {
        // Get to Settlement by having one player fold.
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 500);
        join_player(&s, table_id, &p2, 500);

        s.client.start_hand(&table_id);
        commit_mock_deal(&s, table_id, 2);

        // One player folds -> Settlement
        let table = s.client.get_table(&table_id);
        let current = table.current_turn;
        let folder = table.players.get(current).unwrap();
        s.client
            .player_action(&table_id, &folder.address, &Action::Fold);

        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::Settlement);

        // Now a player can leave
        let winner = if current == 0 {
            table.players.get(1).unwrap()
        } else {
            table.players.get(0).unwrap()
        };
        let winner_stack = winner.stack;

        let withdrawn = s.client.leave_table(&table_id, &winner.address);
        assert_eq!(withdrawn, winner_stack);
        assert_eq!(s.token.balance(&winner.address), winner_stack);
    }

    #[test]
    #[should_panic(expected = "cannot leave during active hand")]
    fn test_cannot_leave_during_active_hand() {
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 500);
        join_player(&s, table_id, &p2, 500);

        s.client.start_hand(&table_id);
        commit_mock_deal(&s, table_id, 2);

        // In Preflop phase, leaving should panic
        s.client.leave_table(&table_id, &p1);
    }

    #[test]
    #[should_panic(expected = "player not at table")]
    fn test_leave_table_not_seated() {
        let s = setup();
        let table_id = create_default_table(&s);
        let stranger = Address::generate(&s.env);
        s.client.leave_table(&table_id, &stranger);
    }

    // ---------------------------------------------------------------------------
    // Additional edge-case tests
    // ---------------------------------------------------------------------------

    #[test]
    fn test_reveal_board_flop() {
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 500);
        join_player(&s, table_id, &p2, 500);

        s.client.start_hand(&table_id);
        commit_mock_deal(&s, table_id, 2);

        // SB calls -> all bets match -> round ends automatically
        let table = s.client.get_table(&table_id);
        let c = table.current_turn;
        let actor = table.players.get(c).unwrap();
        s.client
            .player_action(&table_id, &actor.address, &Action::Call);

        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::DealingFlop);

        // Reveal flop
        let cards: Vec<u32> = Vec::from_array(&s.env, [10, 20, 30]);
        let indices: Vec<u32> = Vec::from_array(&s.env, [4, 5, 6]);
        let proof = soroban_sdk::Bytes::new(&s.env);
        let pub_in = soroban_sdk::Bytes::new(&s.env);
        s.client.reveal_board(
            &table_id,
            &s.committee,
            &cards,
            &indices,
            &proof,
            &pub_in,
        );

        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::Flop);
        assert_eq!(table.board_cards.len(), 3);
        assert_eq!(table.board_cards.get(0).unwrap(), 10);
        assert_eq!(table.board_cards.get(1).unwrap(), 20);
        assert_eq!(table.board_cards.get(2).unwrap(), 30);

        // Bets should be reset
        for i in 0..table.players.len() {
            let p = table.players.get(i).unwrap();
            assert_eq!(p.bet_this_round, 0);
        }
    }

    #[test]
    fn test_timeout_auto_folds_player() {
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        let p3 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 500);
        join_player(&s, table_id, &p2, 500);
        join_player(&s, table_id, &p3, 500);

        s.client.start_hand(&table_id);
        commit_mock_deal(&s, table_id, 3);

        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::Preflop);
        let stalling_seat = table.current_turn;
        let stalling_player = table.players.get(stalling_seat).unwrap();

        // Advance the ledger past the timeout
        let new_seq = table.last_action_ledger + table.config.timeout_ledgers;
        s.env.ledger().set_sequence_number(new_seq);

        // Claim timeout
        let claimer = Address::generate(&s.env);
        s.client.claim_timeout(&table_id, &claimer);

        // The stalling player should be auto-folded
        let table = s.client.get_table(&table_id);
        let folded = table.players.get(stalling_seat).unwrap();
        assert!(folded.folded);
        // Current turn should have advanced
        assert_ne!(table.current_turn, stalling_seat);

        // Verify the stalling player address matches
        assert_eq!(folded.address, stalling_player.address);
    }

    #[test]
    fn test_second_hand_after_settlement() {
        let s = setup();
        let table_id = create_default_table(&s);

        let p1 = Address::generate(&s.env);
        let p2 = Address::generate(&s.env);
        join_player(&s, table_id, &p1, 500);
        join_player(&s, table_id, &p2, 500);

        // Hand 1
        s.client.start_hand(&table_id);
        commit_mock_deal(&s, table_id, 2);

        // One folds -> Settlement
        let table = s.client.get_table(&table_id);
        let c = table.current_turn;
        let folder = table.players.get(c).unwrap();
        s.client
            .player_action(&table_id, &folder.address, &Action::Fold);

        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::Settlement);
        assert_eq!(table.hand_number, 1);

        // Hand 2
        s.client.start_hand(&table_id);
        let table = s.client.get_table(&table_id);
        assert_eq!(table.phase, GamePhase::Dealing);
        assert_eq!(table.hand_number, 2);
        assert_eq!(table.pot, 15); // blinds posted again

        // Verify all players are reset
        for i in 0..table.players.len() {
            let p = table.players.get(i).unwrap();
            assert!(!p.folded);
            assert!(!p.all_in);
        }
    }
}
