use soroban_sdk::{Env, Symbol, Vec};

use crate::game_hub;
use crate::types::*;

/// Initialize state for a new hand.
pub fn start_new_hand(env: &Env, table: &mut TableState) -> Result<(), PokerTableError> {
    table.hand_number += 1;

    // Rotate dealer button
    let num_players = table.players.len() as u32;
    if num_players < 2 {
        return Err(PokerTableError::NeedAtLeastTwoPlayers);
    }
    table.dealer_seat = (table.dealer_seat + 1) % num_players;

    // Reset player states
    for i in 0..table.players.len() {
        let mut p = table
            .players
            .get(i)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;
        p.folded = false;
        p.all_in = false;
        p.bet_this_round = 0;
        table.players.set(i, p);
    }

    // Post blinds
    let sb_seat = (table.dealer_seat + 1) % num_players;
    let bb_seat = (table.dealer_seat + 2) % num_players;

    post_blind(table, sb_seat, table.config.small_blind)?;
    post_blind(table, bb_seat, table.config.big_blind)?;

    // Clear board state
    table.board_cards = Vec::new(env);
    table.dealt_indices = Vec::new(env);
    table.hand_commitments = Vec::new(env);
    table.side_pots = Vec::new(env);

    // Transition to dealing phase (committee will shuffle + deal)
    table.phase = GamePhase::Dealing;
    table.last_action_ledger = env.ledger().sequence();
    Ok(())
}

fn post_blind(table: &mut TableState, seat: u32, amount: i128) -> Result<(), PokerTableError> {
    let mut player = table
        .players
        .get(seat)
        .ok_or(PokerTableError::InvalidPlayerIndex)?;
    let actual = if player.stack < amount {
        player.all_in = true;
        player.stack
    } else {
        amount
    };

    player.stack -= actual;
    player.bet_this_round = actual;
    table.pot += actual;
    table.players.set(seat, player);
    Ok(())
}

/// Count players still active (not folded).
pub fn active_player_count(table: &TableState) -> u32 {
    let mut count = 0u32;
    for i in 0..table.players.len() {
        if let Some(p) = table.players.get(i) {
            if !p.folded {
                count += 1;
            }
        }
    }
    count
}

/// Find the single remaining player (when all others folded).
pub fn last_player_standing(table: &TableState) -> Option<u32> {
    if active_player_count(table) != 1 {
        return None;
    }
    for i in 0..table.players.len() {
        if let Some(p) = table.players.get(i) {
            if !p.folded {
                return Some(p.seat_index);
            }
        }
    }
    None
}

/// Settle the showdown: evaluate hands and distribute pot.
pub fn settle_showdown(
    env: &Env,
    table: &mut TableState,
    hole_cards: &Vec<(u32, u32)>,
) -> Result<(), PokerTableError> {
    // Collect active players' hands
    let mut best_rank: u32 = 0;
    let mut winner_seat: u32 = 0;

    let board = &table.board_cards;
    if board.len() != 5 {
        return Err(PokerTableError::BoardNotComplete);
    }

    let board_arr: [u32; 5] = [
        board.get(0).ok_or(PokerTableError::BoardNotComplete)?,
        board.get(1).ok_or(PokerTableError::BoardNotComplete)?,
        board.get(2).ok_or(PokerTableError::BoardNotComplete)?,
        board.get(3).ok_or(PokerTableError::BoardNotComplete)?,
        board.get(4).ok_or(PokerTableError::BoardNotComplete)?,
    ];

    let mut active_idx = 0u32;
    for i in 0..table.players.len() {
        let p = table
            .players
            .get(i)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;
        if p.folded {
            continue;
        }

        let (c1, c2) = hole_cards
            .get(active_idx)
            .ok_or(PokerTableError::InvalidHoleCards)?;
        let cards: [u32; 7] = [
            c1,
            c2,
            board_arr[0],
            board_arr[1],
            board_arr[2],
            board_arr[3],
            board_arr[4],
        ];

        let rank = stellar_zk_cards::evaluate_hand(&cards);
        if rank.score > best_rank {
            best_rank = rank.score;
            winner_seat = p.seat_index;
        }

        active_idx += 1;
    }

    // Award pot to winner
    let winnings = table.pot;
    let mut winner = table
        .players
        .get(winner_seat)
        .ok_or(PokerTableError::InvalidPlayerIndex)?;
    winner.stack += winnings;
    table.players.set(winner_seat, winner.clone());
    table.pot = 0;

    table.phase = GamePhase::Settlement;
    table.last_action_ledger = env.ledger().sequence();

    // Notify game hub: player1_won = true if winner is seat 0 (player1)
    let player1_won = winner_seat == 0;
    game_hub::notify_end(env, &table.config.game_hub, table.session_id, player1_won);

    env.events().publish(
        (Symbol::new(env, "hand_settled"), table.id),
        (winner.address.clone(), winnings),
    );
    Ok(())
}

/// Award pot to last player standing (all others folded).
pub fn settle_fold_win(env: &Env, table: &mut TableState) -> Result<(), PokerTableError> {
    if let Some(winner_seat) = last_player_standing(table) {
        let winnings = table.pot;
        let mut winner = table
            .players
            .get(winner_seat)
            .ok_or(PokerTableError::InvalidPlayerIndex)?;
        winner.stack += winnings;
        table.players.set(winner_seat, winner.clone());
        table.pot = 0;
        table.phase = GamePhase::Settlement;
        table.last_action_ledger = env.ledger().sequence();

        // Notify game hub
        let player1_won = winner_seat == 0;
        game_hub::notify_end(env, &table.config.game_hub, table.session_id, player1_won);

        env.events().publish(
            (Symbol::new(env, "fold_win"), table.id),
            (winner.address.clone(), winnings),
        );
    }
    Ok(())
}
