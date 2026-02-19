use soroban_sdk::{Address, Env, Symbol};

use crate::game;
use crate::types::*;

/// Process a timeout claim.
/// Anyone can call this if enough ledgers have passed since the last action.
pub fn process_timeout(
    env: &Env,
    table: &mut TableState,
    _claimer: &Address,
) -> Result<(), PokerTableError> {
    let current_ledger = env.ledger().sequence();
    let elapsed = current_ledger - table.last_action_ledger;

    if elapsed < table.config.timeout_ledgers {
        return Err(PokerTableError::TimeoutNotReached);
    }

    match table.phase {
        // Player timeout during betting — auto-fold the stalling player
        GamePhase::Preflop | GamePhase::Flop | GamePhase::Turn | GamePhase::River => {
            let seat = table.current_turn;
            let mut p = table
                .players
                .get(seat)
                .ok_or(PokerTableError::InvalidPlayerIndex)?;

            if !p.folded && !p.all_in {
                p.folded = true;
                table.players.set(seat, p.clone());

                env.events().publish(
                    (Symbol::new(env, "timeout_fold"), table.id),
                    p.address.clone(),
                );

                // Check if only one player remains
                if game::active_player_count(table) == 1 {
                    game::settle_fold_win(env, table)?;
                } else {
                    // Advance to next player
                    let num_players = table.players.len() as u32;
                    let mut next = (seat + 1) % num_players;
                    for _ in 0..num_players {
                        let np = table
                            .players
                            .get(next)
                            .ok_or(PokerTableError::InvalidPlayerIndex)?;
                        if !np.folded && !np.all_in {
                            break;
                        }
                        next = (next + 1) % num_players;
                    }
                    table.current_turn = next;
                    table.last_action_ledger = current_ledger;
                }
            }
        }

        // Committee timeout during dealing/reveal — dispute, return funds
        GamePhase::Dealing
        | GamePhase::DealingFlop
        | GamePhase::DealingTurn
        | GamePhase::DealingRiver
        | GamePhase::Showdown => {
            // Committee failed to act — enter dispute phase
            table.phase = GamePhase::Dispute;
            table.last_action_ledger = current_ledger;

            env.events().publish(
                (Symbol::new(env, "committee_timeout"), table.id),
                table.hand_number,
            );

            // Return all funds to players (emergency settlement)
            emergency_refund(env, table)?;
        }

        _ => {
            return Err(PokerTableError::TimeoutNotApplicable);
        }
    }
    Ok(())
}

/// Emergency refund: return all player stacks + pot split equally
/// among non-folded players. Used when committee fails.
fn emergency_refund(_env: &Env, table: &mut TableState) -> Result<(), PokerTableError> {
    let active = game::active_player_count(table);
    if active == 0 {
        return Ok(());
    }

    let share = table.pot / (active as i128);
    let mut distributed: i128 = 0;

    for i in 0..table.players.len() {
        let mut p = table
            .players
            .get(i)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;
        if !p.folded {
            p.stack += share;
            distributed += share;
        }
        table.players.set(i, p);
    }

    // Handle remainder (give to first active player)
    let remainder = table.pot - distributed;
    if remainder > 0 {
        for i in 0..table.players.len() {
            let mut p = table
                .players
                .get(i)
                .ok_or(PokerTableError::InvalidPlayerIndex)?;
            if !p.folded {
                p.stack += remainder;
                table.players.set(i, p);
                break;
            }
        }
    }

    table.pot = 0;
    table.phase = GamePhase::Settlement;
    Ok(())
}
