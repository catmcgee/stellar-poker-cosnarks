/// Board Reveal Validity Circuit
///
/// Proves that revealed community cards (flop/turn/river) are consistent
/// with the committed deck and haven't been previously dealt.
///
/// This circuit is proven inside MPC to reveal board cards while keeping
/// the rest of the deck secret.
///
/// Public inputs:
///   - deck_root: Merkle root of the committed deck
///   - revealed_cards: plaintext card values being revealed
///   - revealed_indices: deck positions of revealed cards
///   - previously_used_indices: indices already dealt (to players or board)
///
/// Private inputs:
///   - deck: full shuffled deck
///   - salts: per-card randomness
///   - merkle_proofs: Merkle proofs for each revealed card

use stellar_poker_lib::cards;
use stellar_poker_lib::commitments;
use stellar_poker_lib::merkle;

/// Maximum cards that can be revealed at once (flop = 3)
global MAX_REVEAL: u32 = 3;
/// Maximum previously used indices (2 per player * 6 players + 4 board = 16)
global MAX_USED: u32 = 16;

fn main(
    // Private inputs
    deck: [Field; 52],
    salts: [Field; 52],

    // Public inputs
    deck_root: pub Field,
    num_revealed: pub u32,
    revealed_cards: pub [Field; MAX_REVEAL],
    revealed_indices: pub [u32; MAX_REVEAL],
    num_previously_used: pub u32,
    previously_used_indices: pub [u32; MAX_USED],
) {
    // 1. Recompute deck Merkle root and verify
    let mut leaves: [Field; 64] = [0; 64];
    for i in 0..52 {
        leaves[i] = commitments::commit_card(deck[i], salts[i]);
    }
    let computed_root = merkle::compute_merkle_root(leaves);
    assert(computed_root == deck_root, "deck root mismatch");

    // 2. For each revealed card, verify it matches the deck at the given index
    for i in 0..MAX_REVEAL {
        if i < num_revealed {
            let idx = revealed_indices[i];
            assert(idx < 52, "reveal index out of range");

            // Verify the revealed card matches the deck
            assert(
                deck[idx] == revealed_cards[i],
                "revealed card doesn't match deck",
            );

            // Verify the card is valid
            cards::assert_valid_card(revealed_cards[i]);
        }
    }

    // 3. Verify no revealed index was previously used
    for i in 0..MAX_REVEAL {
        if i < num_revealed {
            for j in 0..MAX_USED {
                if j < num_previously_used {
                    assert(
                        revealed_indices[i] != previously_used_indices[j],
                        "card already dealt",
                    );
                }
            }
        }
    }

    // 4. Verify no duplicate revealed indices
    for i in 0..MAX_REVEAL {
        if i < num_revealed {
            for j in (i + 1)..MAX_REVEAL {
                if j < num_revealed {
                    assert(
                        revealed_indices[i] != revealed_indices[j],
                        "duplicate reveal index",
                    );
                }
            }
        }
    }
}
