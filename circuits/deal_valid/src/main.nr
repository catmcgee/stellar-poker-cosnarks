/// Deal Validity Circuit
///
/// Proves that from a committed deck, specific cards were dealt to specific
/// players, and the hand commitments match.
///
/// This circuit is proven inside MPC (via TACEO coNoir) so that no single
/// party knows the full deck or all players' cards.
///
/// Public inputs:
///   - deck_root: Merkle root of the committed deck
///   - hand_commitments: Poseidon2 hash of each player's hole cards
///   - dealt_indices: which deck positions were dealt to each player
///
/// Private inputs (secret-shared in MPC):
///   - deck: the full shuffled deck (52 card values)
///   - salts: per-card randomness for commitments
///   - player_cards: each player's 2 hole cards

use stellar_poker_lib::cards;
use stellar_poker_lib::commitments;
use stellar_poker_lib::merkle;

/// Maximum players supported (for fixed circuit size)
global MAX_PLAYERS: u32 = 6;

fn main(
    // Private inputs (secret-shared in MPC)
    deck: [Field; 52],
    salts: [Field; 52],

    // Public inputs
    deck_root: pub Field,
    num_players: pub u32,
    hand_commitments: pub [Field; MAX_PLAYERS],
    dealt_card1_indices: pub [u32; MAX_PLAYERS],
    dealt_card2_indices: pub [u32; MAX_PLAYERS],
) {
    // 1. Verify all cards in deck are valid and unique
    cards::assert_valid_deck(deck);

    // 2. Compute card commitments and verify Merkle root
    let mut leaves: [Field; 64] = [0; 64];
    for i in 0..52 {
        leaves[i] = commitments::commit_card(deck[i], salts[i]);
    }
    // Padding leaves 52-63 are zero

    let computed_root = merkle::compute_merkle_root(leaves);
    assert(computed_root == deck_root, "deck root mismatch");

    // 3. Verify each player's hand commitment
    for p in 0..MAX_PLAYERS {
        if p < num_players {
            let idx1 = dealt_card1_indices[p];
            let idx2 = dealt_card2_indices[p];

            // Cards must be within deck range
            assert(idx1 < 52, "card index out of range");
            assert(idx2 < 52, "card index out of range");

            // Indices must be different
            assert(idx1 != idx2, "same card dealt twice to player");

            // Compute expected hand commitment
            let card1_commit = commitments::commit_card(deck[idx1], salts[idx1]);
            let card2_commit = commitments::commit_card(deck[idx2], salts[idx2]);
            let expected_hand_commit = commitments::commit_hand(card1_commit, card2_commit);

            assert(
                expected_hand_commit == hand_commitments[p],
                "hand commitment mismatch",
            );
        }
    }

    // 4. Verify no card index is dealt to multiple players
    for p1 in 0..MAX_PLAYERS {
        if p1 < num_players {
            for p2 in (p1 + 1)..MAX_PLAYERS {
                if p2 < num_players {
                    assert(
                        dealt_card1_indices[p1] != dealt_card1_indices[p2],
                        "same card dealt to multiple players",
                    );
                    assert(
                        dealt_card1_indices[p1] != dealt_card2_indices[p2],
                        "same card dealt to multiple players",
                    );
                    assert(
                        dealt_card2_indices[p1] != dealt_card1_indices[p2],
                        "same card dealt to multiple players",
                    );
                    assert(
                        dealt_card2_indices[p1] != dealt_card2_indices[p2],
                        "same card dealt to multiple players",
                    );
                }
            }
        }
    }
}
