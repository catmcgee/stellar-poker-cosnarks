//! Deck management for the MPC committee.
//!
//! Hackathon-pragmatic shuffle approach:
//! The coordinator generates the shuffled deck + salts centrally, then immediately
//! splits them into secret shares via `co-noir split-input` and distributes to nodes.
//! The plaintext stays in coordinator memory only for card delivery.
//!
//! Commitments and Merkle root are computed using real Poseidon2 hashing
//! (matching the Noir circuit) so that the public inputs in the Prover.toml
//! are correct and the circuit constraints will be satisfied.

use ark_bn254::Fr;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

use crate::crypto;

pub const DECK_SIZE: usize = 52;

/// Deck state held by the coordinator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeckState {
    /// Shuffled card values (0-51). SECRET — only exists in MPC shares.
    pub cards: Vec<u32>,
    /// Per-card random salts for Poseidon2 commitments. SECRET.
    pub salts: Vec<String>,
    /// Per-card Poseidon2 commitments as decimal strings. PUBLIC.
    pub commitments: Vec<String>,
    /// Merkle root of commitments (decimal string). PUBLIC.
    pub merkle_root: String,
}

/// Create a new shuffled deck with real Poseidon2 commitments.
pub fn shuffle_deck_dev() -> DeckState {
    let mut rng = thread_rng();

    let mut cards: Vec<u32> = (0..DECK_SIZE as u32).collect();
    cards.shuffle(&mut rng);

    // Generate random salts
    let salts: Vec<String> = (0..DECK_SIZE)
        .map(|_| format!("{}", rand::random::<u64>()))
        .collect();

    // Compute real Poseidon2 commitments matching the Noir circuit
    let commitment_frs: Vec<Fr> = cards
        .iter()
        .zip(salts.iter())
        .map(|(card, salt)| crypto::commit_card(*card, salt))
        .collect();

    let commitments: Vec<String> = commitment_frs
        .iter()
        .map(|c| crypto::fr_to_decimal_string(c))
        .collect();

    // Build 64-leaf array (52 commitments + 12 zero padding)
    let mut leaves = [Fr::from(0u64); 64];
    for (i, c) in commitment_frs.iter().enumerate() {
        leaves[i] = *c;
    }

    let root = crypto::compute_merkle_root(&leaves);
    let merkle_root = crypto::fr_to_decimal_string(&root);

    DeckState {
        cards,
        salts,
        commitments,
        merkle_root,
    }
}

/// Compute the hand commitment for two card indices.
/// Returns the commitment as a decimal string.
pub fn compute_hand_commitment(deck: &DeckState, idx1: u32, idx2: u32) -> String {
    let c1 = crypto::commit_card(deck.cards[idx1 as usize], &deck.salts[idx1 as usize]);
    let c2 = crypto::commit_card(deck.cards[idx2 as usize], &deck.salts[idx2 as usize]);
    let hand = crypto::commit_hand(c1, c2);
    crypto::fr_to_decimal_string(&hand)
}

/// Get cards at specific deck positions for a player.
#[allow(dead_code)]
pub fn get_player_hole_cards(deck: &DeckState, idx1: usize, idx2: usize) -> (u32, u32) {
    (deck.cards[idx1], deck.cards[idx2])
}

/// Get the next N undealt card indices from the deck.
pub fn next_card_indices(dealt: &[u32], count: usize) -> Vec<u32> {
    let next_start = if dealt.is_empty() {
        0
    } else {
        dealt.iter().max().unwrap() + 1
    };

    (next_start..next_start + count as u32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shuffle_produces_valid_deck() {
        let deck = shuffle_deck_dev();
        assert_eq!(deck.cards.len(), DECK_SIZE);

        let mut sorted = deck.cards.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), DECK_SIZE);

        assert!(deck.cards.iter().all(|&c| c < DECK_SIZE as u32));
    }

    #[test]
    fn test_shuffle_produces_real_commitments() {
        let deck = shuffle_deck_dev();
        // Commitments should be decimal strings (not placeholder format)
        assert!(!deck.commitments[0].starts_with("commit_"));
        assert!(!deck.merkle_root.starts_with("merkle_root_"));
        // All commitments should be non-empty
        assert!(deck.commitments.iter().all(|c| !c.is_empty()));
    }

    #[test]
    fn test_hand_commitment_deterministic() {
        let deck = shuffle_deck_dev();
        let hc1 = compute_hand_commitment(&deck, 0, 1);
        let hc2 = compute_hand_commitment(&deck, 0, 1);
        assert_eq!(hc1, hc2);
    }

    #[test]
    fn test_next_card_indices() {
        let dealt = vec![0, 1, 2, 3];
        let flop = next_card_indices(&dealt, 3);
        assert_eq!(flop, vec![4, 5, 6]);

        let turn = next_card_indices(&[0, 1, 2, 3, 4, 5, 6], 1);
        assert_eq!(turn, vec![7]);
    }
}
