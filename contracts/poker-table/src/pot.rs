use soroban_sdk::{Env, Vec};

use crate::types::*;

/// Calculate side pots when players are all-in with different amounts.
/// This is simplified for v1 — handles the common case of one main pot
/// and one side pot.
#[allow(dead_code)]
pub fn calculate_side_pots(env: &Env, table: &TableState) -> Vec<SidePot> {
    let mut pots: Vec<SidePot> = Vec::new(env);

    // Collect all-in amounts and sort
    let mut all_in_levels: Vec<i128> = Vec::new(env);
    for i in 0..table.players.len() {
        let p = table.players.get(i).unwrap();
        if p.all_in && p.bet_this_round > 0 {
            // Insert sorted
            let mut inserted = false;
            for j in 0..all_in_levels.len() {
                if p.bet_this_round <= all_in_levels.get(j).unwrap() {
                    if p.bet_this_round < all_in_levels.get(j).unwrap() {
                        all_in_levels.insert(j, p.bet_this_round);
                    }
                    inserted = true;
                    break;
                }
            }
            if !inserted {
                all_in_levels.push_back(p.bet_this_round);
            }
        }
    }

    if all_in_levels.is_empty() {
        // No side pots needed — single main pot
        let mut eligible = Vec::new(env);
        for i in 0..table.players.len() {
            let p = table.players.get(i).unwrap();
            if !p.folded {
                eligible.push_back(p.seat_index);
            }
        }
        pots.push_back(SidePot {
            amount: table.pot,
            eligible_players: eligible,
        });
        return pots;
    }

    // Build pots at each all-in level
    let mut prev_level: i128 = 0;
    for lvl_idx in 0..all_in_levels.len() {
        let level = all_in_levels.get(lvl_idx).unwrap();
        let _increment = level - prev_level;
        let mut pot_amount: i128 = 0;
        let mut eligible = Vec::new(env);

        for i in 0..table.players.len() {
            let p = table.players.get(i).unwrap();
            if p.folded {
                continue;
            }
            let contributed = core::cmp::min(p.bet_this_round, level) - core::cmp::min(p.bet_this_round, prev_level);
            pot_amount += contributed;
            if p.bet_this_round >= level {
                eligible.push_back(p.seat_index);
            }
        }

        if pot_amount > 0 {
            pots.push_back(SidePot {
                amount: pot_amount,
                eligible_players: eligible,
            });
        }
        prev_level = level;
    }

    // Remaining pot for players who bet more than highest all-in
    let max_level = all_in_levels.get(all_in_levels.len() - 1).unwrap();
    let mut remaining: i128 = 0;
    let mut eligible = Vec::new(env);
    for i in 0..table.players.len() {
        let p = table.players.get(i).unwrap();
        if p.folded {
            continue;
        }
        if p.bet_this_round > max_level {
            remaining += p.bet_this_round - max_level;
            eligible.push_back(p.seat_index);
        }
    }
    if remaining > 0 {
        pots.push_back(SidePot {
            amount: remaining,
            eligible_players: eligible,
        });
    }

    pots
}
