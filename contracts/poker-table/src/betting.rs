use soroban_sdk::{Address, Env, Symbol};

use crate::game;
use crate::types::*;

/// Process a player's betting action.
pub fn process_action(env: &Env, table: &mut TableState, player: &Address, action: &Action) {
    // Find the player
    let seat = find_player_seat(table, player);
    assert!(seat == table.current_turn, "not your turn");

    let mut p = table.players.get(seat).unwrap();
    assert!(!p.folded, "player has folded");
    assert!(!p.all_in, "player is all-in");

    let current_bet = max_bet_this_round(table);

    match action {
        Action::Fold => {
            p.folded = true;
            table.players.set(seat, p);

            // Check if only one player remains
            if game::active_player_count(table) == 1 {
                game::settle_fold_win(env, table);
                return;
            }
        }
        Action::Check => {
            assert!(
                p.bet_this_round == current_bet,
                "must call or fold"
            );
        }
        Action::Call => {
            let to_call = current_bet - p.bet_this_round;
            assert!(to_call > 0, "nothing to call");
            let actual = core::cmp::min(to_call, p.stack);

            p.stack -= actual;
            p.bet_this_round += actual;
            table.pot += actual;

            if p.stack == 0 {
                p.all_in = true;
            }
            table.players.set(seat, p);
        }
        Action::Bet(amount) => {
            assert!(current_bet == 0, "cannot bet; use raise");
            assert!(*amount >= table.config.big_blind, "bet too small");
            assert!(*amount <= p.stack, "not enough chips");

            p.stack -= *amount;
            p.bet_this_round += *amount;
            table.pot += *amount;

            if p.stack == 0 {
                p.all_in = true;
            }
            table.players.set(seat, p);
        }
        Action::Raise(amount) => {
            let to_call = current_bet - p.bet_this_round;
            let total_needed = to_call + *amount;
            assert!(*amount >= table.config.big_blind, "raise too small");
            assert!(total_needed <= p.stack, "not enough chips");

            p.stack -= total_needed;
            p.bet_this_round += total_needed;
            table.pot += total_needed;

            if p.stack == 0 {
                p.all_in = true;
            }
            table.players.set(seat, p);
        }
        Action::AllIn => {
            let amount = p.stack;
            p.bet_this_round += amount;
            table.pot += amount;
            p.stack = 0;
            p.all_in = true;
            table.players.set(seat, p);
        }
    }

    table.last_action_ledger = env.ledger().sequence();

    // Advance turn
    advance_turn(env, table);
}

/// Reset betting state for a new round.
pub fn reset_round(env: &Env, table: &mut TableState) {
    for i in 0..table.players.len() {
        let mut p = table.players.get(i).unwrap();
        p.bet_this_round = 0;
        table.players.set(i, p);
    }

    // First active player after dealer acts first post-flop
    let num_players = table.players.len() as u32;
    let mut seat = (table.dealer_seat + 1) % num_players;
    for _ in 0..num_players {
        let p = table.players.get(seat).unwrap();
        if !p.folded && !p.all_in {
            table.current_turn = seat;
            return;
        }
        seat = (seat + 1) % num_players;
    }

    // All players are all-in or folded — skip to next deal phase
    advance_to_next_phase(env, table);
}

/// Advance to the next player's turn, or end the betting round.
fn advance_turn(env: &Env, table: &mut TableState) {
    let num_players = table.players.len() as u32;
    let mut next = (table.current_turn + 1) % num_players;

    // Find next active player
    for _ in 0..num_players {
        let p = table.players.get(next).unwrap();
        if !p.folded && !p.all_in {
            break;
        }
        next = (next + 1) % num_players;
    }

    // Check if betting round is complete
    if is_round_complete(table) {
        advance_to_next_phase(env, table);
    } else {
        table.current_turn = next;
    }
}

/// Check if all active players have matched the current bet.
fn is_round_complete(table: &TableState) -> bool {
    let current_bet = max_bet_this_round(table);
    for i in 0..table.players.len() {
        let p = table.players.get(i).unwrap();
        if p.folded || p.all_in {
            continue;
        }
        if p.bet_this_round != current_bet {
            return false;
        }
    }

    // All active non-all-in players have matched the current bet
    true
}

/// Advance to the next game phase.
fn advance_to_next_phase(env: &Env, table: &mut TableState) {
    // If only one player left, settle immediately
    if game::active_player_count(table) == 1 {
        game::settle_fold_win(env, table);
        return;
    }

    table.phase = match table.phase {
        GamePhase::Preflop => GamePhase::DealingFlop,
        GamePhase::Flop => GamePhase::DealingTurn,
        GamePhase::Turn => GamePhase::DealingRiver,
        GamePhase::River => GamePhase::Showdown,
        _ => return,
    };
    table.last_action_ledger = env.ledger().sequence();

    env.events().publish(
        (Symbol::new(env, "phase_change"), table.id),
        table.phase.clone(),
    );
}

fn find_player_seat(table: &TableState, player: &Address) -> u32 {
    for i in 0..table.players.len() {
        let p = table.players.get(i).unwrap();
        if p.address == *player {
            return p.seat_index;
        }
    }
    panic!("player not at table");
}

fn max_bet_this_round(table: &TableState) -> i128 {
    let mut max_bet: i128 = 0;
    for i in 0..table.players.len() {
        let p = table.players.get(i).unwrap();
        if p.bet_this_round > max_bet {
            max_bet = p.bet_this_round;
        }
    }
    max_bet
}
