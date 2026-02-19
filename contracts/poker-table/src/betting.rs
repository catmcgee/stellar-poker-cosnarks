use soroban_sdk::{Address, Env, Symbol};

use crate::game;
use crate::types::*;

/// Process a player's betting action.
pub fn process_action(
    env: &Env,
    table: &mut TableState,
    player: &Address,
    action: &Action,
) -> Result<(), PokerTableError> {
    // Find the player
    let seat = find_player_seat(table, player)?;
    if seat != table.current_turn {
        return Err(PokerTableError::NotYourTurn);
    }

    let mut p = table
        .players
        .get(seat)
        .ok_or(PokerTableError::InvalidPlayerIndex)?;
    if p.folded {
        return Err(PokerTableError::PlayerAlreadyFolded);
    }
    if p.all_in {
        return Err(PokerTableError::PlayerAlreadyAllIn);
    }

    let current_bet = max_bet_this_round(table)?;

    match action {
        Action::Fold => {
            p.folded = true;
            table.players.set(seat, p);

            // Check if only one player remains
            if game::active_player_count(table) == 1 {
                game::settle_fold_win(env, table)?;
                return Ok(());
            }
        }
        Action::Check => {
            if p.bet_this_round != current_bet {
                return Err(PokerTableError::MustCallOrFold);
            }
        }
        Action::Call => {
            let to_call = current_bet - p.bet_this_round;
            if to_call <= 0 {
                return Err(PokerTableError::NothingToCall);
            }
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
            if current_bet != 0 {
                return Err(PokerTableError::CannotBetWhenOutstandingBet);
            }
            if *amount < table.config.big_blind {
                return Err(PokerTableError::BetTooSmall);
            }
            if *amount > p.stack {
                return Err(PokerTableError::NotEnoughChips);
            }

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
            if *amount < table.config.big_blind {
                return Err(PokerTableError::RaiseTooSmall);
            }
            if total_needed > p.stack {
                return Err(PokerTableError::NotEnoughChips);
            }

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
    advance_turn(env, table)
}

/// Reset betting state for a new round.
pub fn reset_round(env: &Env, table: &mut TableState) -> Result<(), PokerTableError> {
    for i in 0..table.players.len() {
        let mut p = table
            .players
            .get(i)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;
        p.bet_this_round = 0;
        table.players.set(i, p);
    }

    // First active player after dealer acts first post-flop
    let num_players = table.players.len() as u32;
    if num_players == 0 {
        return Err(PokerTableError::NeedAtLeastTwoPlayers);
    }
    let mut seat = (table.dealer_seat + 1) % num_players;
    for _ in 0..num_players {
        let p = table
            .players
            .get(seat)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;
        if !p.folded && !p.all_in {
            table.current_turn = seat;
            return Ok(());
        }
        seat = (seat + 1) % num_players;
    }

    // All players are all-in or folded — skip to next deal phase
    advance_to_next_phase(env, table)
}

/// Advance to the next player's turn, or end the betting round.
fn advance_turn(env: &Env, table: &mut TableState) -> Result<(), PokerTableError> {
    let num_players = table.players.len() as u32;
    if num_players == 0 {
        return Err(PokerTableError::NeedAtLeastTwoPlayers);
    }
    let mut next = (table.current_turn + 1) % num_players;

    // Find next active player
    for _ in 0..num_players {
        let p = table
            .players
            .get(next)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;
        if !p.folded && !p.all_in {
            break;
        }
        next = (next + 1) % num_players;
    }

    // Check if betting round is complete
    if is_round_complete(table)? {
        advance_to_next_phase(env, table)?;
    } else {
        table.current_turn = next;
    }
    Ok(())
}

/// Check if all active players have matched the current bet.
fn is_round_complete(table: &TableState) -> Result<bool, PokerTableError> {
    let current_bet = max_bet_this_round(table)?;
    for i in 0..table.players.len() {
        let p = table
            .players
            .get(i)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;
        if p.folded || p.all_in {
            continue;
        }
        if p.bet_this_round != current_bet {
            return Ok(false);
        }
    }

    // All active non-all-in players have matched the current bet
    Ok(true)
}

/// Advance to the next game phase.
fn advance_to_next_phase(env: &Env, table: &mut TableState) -> Result<(), PokerTableError> {
    // If only one player left, settle immediately
    if game::active_player_count(table) == 1 {
        game::settle_fold_win(env, table)?;
        return Ok(());
    }

    table.phase = match table.phase {
        GamePhase::Preflop => GamePhase::DealingFlop,
        GamePhase::Flop => GamePhase::DealingTurn,
        GamePhase::Turn => GamePhase::DealingRiver,
        GamePhase::River => GamePhase::Showdown,
        _ => return Ok(()),
    };
    table.last_action_ledger = env.ledger().sequence();

    env.events().publish(
        (Symbol::new(env, "phase_change"), table.id),
        table.phase.clone(),
    );
    Ok(())
}

fn find_player_seat(table: &TableState, player: &Address) -> Result<u32, PokerTableError> {
    for i in 0..table.players.len() {
        let p = table
            .players
            .get(i)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;
        if p.address == *player {
            return Ok(p.seat_index);
        }
    }
    Err(PokerTableError::PlayerNotAtTable)
}

fn max_bet_this_round(table: &TableState) -> Result<i128, PokerTableError> {
    let mut max_bet: i128 = 0;
    for i in 0..table.players.len() {
        let p = table
            .players
            .get(i)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;
        if p.bet_this_round > max_bet {
            max_bet = p.bet_this_round;
        }
    }
    Ok(max_bet)
}
