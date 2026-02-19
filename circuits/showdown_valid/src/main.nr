/// Showdown Validity Circuit
///
/// Proves that the declared winner of the hand is correct given:
/// - Each player's hole cards (revealed at showdown in v1)
/// - The community board cards (already public)
/// - The hand commitments (previously posted on-chain)
///
/// V1: Public showdown (hole cards are revealed, commitment consistency verified).
/// V2 (future): Privacy-preserving showdown using ZK hand ranking comparison.
///
/// Public inputs:
///   - hand_commitments: per-player commitments from deal phase
///   - board_cards: 5 community cards
///   - hole_cards + salts: revealed per-player cards (v1)
///   - winner_index: claimed winner
///
/// The circuit verifies:
///   1. Revealed cards match the original commitments
///   2. Hand evaluation is correct
///   3. Winner has the best hand

use stellar_poker_lib::cards;
use stellar_poker_lib::commitments;

global MAX_PLAYERS: u32 = 6;

fn main(
    // Public inputs
    num_active_players: pub u32,
    hand_commitments: pub [Field; MAX_PLAYERS],
    board_cards: pub [u32; 5],
    hole_card1: pub [u32; MAX_PLAYERS],
    hole_card2: pub [u32; MAX_PLAYERS],
    salts1: pub [Field; MAX_PLAYERS],
    salts2: pub [Field; MAX_PLAYERS],
    winner_index: pub u32,
) {
    assert(num_active_players >= 2, "need at least 2 players for showdown");
    assert(winner_index < num_active_players, "winner index out of range");

    // 1. Verify all board cards are valid
    for i in 0..5 {
        cards::assert_valid_card(board_cards[i] as Field);
    }

    // 2. For each active player, verify commitment and evaluate hand
    let mut hand_scores: [u32; MAX_PLAYERS] = [0; MAX_PLAYERS];

    for p in 0..MAX_PLAYERS {
        if p < num_active_players {
            // Verify card validity
            cards::assert_valid_card(hole_card1[p] as Field);
            cards::assert_valid_card(hole_card2[p] as Field);

            // Verify hand commitment matches
            let c1_commit = commitments::commit_card(hole_card1[p] as Field, salts1[p]);
            let c2_commit = commitments::commit_card(hole_card2[p] as Field, salts2[p]);
            let expected_commit = commitments::commit_hand(c1_commit, c2_commit);
            assert(
                expected_commit == hand_commitments[p],
                "hand commitment mismatch at showdown",
            );

            // Evaluate hand (2 hole + 5 board = 7 cards)
            let seven_cards: [u32; 7] = [
                hole_card1[p],
                hole_card2[p],
                board_cards[0],
                board_cards[1],
                board_cards[2],
                board_cards[3],
                board_cards[4],
            ];

            hand_scores[p] = cards::evaluate_hand_rank(seven_cards);
        }
    }

    // 3. Verify the declared winner has the highest hand score
    let winner_score = hand_scores[winner_index];

    for p in 0..MAX_PLAYERS {
        if p < num_active_players {
            // Winner's score must be >= all other scores
            assert(
                winner_score >= hand_scores[p],
                "declared winner doesn't have the best hand",
            );
        }
    }
}
