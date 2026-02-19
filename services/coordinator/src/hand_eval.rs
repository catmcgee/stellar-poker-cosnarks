//! Poker hand evaluation matching the Noir circuit's `cards::evaluate_hand_rank`.
//!
//! Card encoding: card_value = suit * 13 + rank
//! - suit: 0=Clubs, 1=Diamonds, 2=Hearts, 3=Spades
//! - rank: 0=2, 1=3, ..., 8=10, 9=J, 10=Q, 11=K, 12=A

const NUM_RANKS: u32 = 13;

/// Evaluate the best 5-card hand from 7 cards (2 hole + 5 board).
/// Returns a score where higher = better hand.
/// Matches the Noir circuit's `evaluate_hand_rank` exactly.
pub fn evaluate_hand_rank(cards: &[u32; 7]) -> u32 {
    let mut best_score: u32 = 0;

    // Check all C(7,5) = 21 combinations by choosing which 2 to skip
    for skip1 in 0..7u32 {
        for skip2 in (skip1 + 1)..7u32 {
            let mut hand = [0u32; 5];
            let mut idx = 0usize;
            for k in 0..7u32 {
                if k != skip1 && k != skip2 {
                    hand[idx] = cards[k as usize];
                    idx += 1;
                }
            }
            let score = score_five(&hand);
            if score > best_score {
                best_score = score;
            }
        }
    }

    best_score
}

/// Score exactly 5 cards. Returns category << 20 | tiebreaker.
/// Matches the Noir circuit's `score_five` exactly.
fn score_five(cards: &[u32; 5]) -> u32 {
    let mut ranks = [0u32; 5];
    let mut suits = [0u32; 5];
    for i in 0..5 {
        ranks[i] = cards[i] % NUM_RANKS;
        suits[i] = cards[i] / NUM_RANKS;
    }

    // Sort ranks descending (bubble sort matching the circuit)
    for i in 0..4 {
        for j in 0..(4 - i) {
            if ranks[j] < ranks[j + 1] {
                ranks.swap(j, j + 1);
            }
        }
    }

    let is_flush = suits[0] == suits[1]
        && suits[1] == suits[2]
        && suits[2] == suits[3]
        && suits[3] == suits[4];

    let is_straight = ranks[0] == ranks[1] + 1
        && ranks[1] == ranks[2] + 1
        && ranks[2] == ranks[3] + 1
        && ranks[3] == ranks[4] + 1;

    // Ace-low straight (A-2-3-4-5 = wheel)
    let is_wheel = ranks[0] == 12
        && ranks[1] == 3
        && ranks[2] == 2
        && ranks[3] == 1
        && ranks[4] == 0;

    // Count frequencies
    let mut freq = [0u32; 13];
    for i in 0..5 {
        freq[ranks[i] as usize] += 1;
    }

    let mut has_four = false;
    let mut has_three = false;
    let mut pair_count: u32 = 0;
    let mut four_rank: u32 = 0;
    let mut three_rank: u32 = 0;
    let mut pair_rank_hi: u32 = 0;
    let mut pair_rank_lo: u32 = 0;

    for r_inv in 0..13u32 {
        let r = 12 - r_inv;
        if freq[r as usize] == 4 {
            has_four = true;
            four_rank = r;
        } else if freq[r as usize] == 3 {
            has_three = true;
            three_rank = r;
        } else if freq[r as usize] == 2 {
            if pair_count == 0 {
                pair_rank_hi = r;
            } else {
                pair_rank_lo = r;
            }
            pair_count += 1;
        }
    }

    let tb = (ranks[0] << 16) | (ranks[1] << 12) | (ranks[2] << 8) | (ranks[3] << 4) | ranks[4];

    // Categorical scoring matching the circuit exactly
    if is_flush && is_straight && ranks[0] == 12 {
        return (9 << 20) | tb; // Royal flush
    }
    if is_flush && (is_straight || is_wheel) {
        let high = if is_wheel { 3 << 16 } else { tb };
        return (8 << 20) | high; // Straight flush
    }
    if has_four {
        return (7 << 20) | (four_rank << 16); // Four of a kind
    }
    if has_three && pair_count >= 1 {
        return (6 << 20) | (three_rank << 8) | pair_rank_hi; // Full house
    }
    if is_flush {
        return (5 << 20) | tb; // Flush
    }
    if is_straight || is_wheel {
        let high = if is_wheel { 3 << 16 } else { ranks[0] << 16 };
        return (4 << 20) | high; // Straight
    }
    if has_three {
        return (3 << 20) | (three_rank << 16); // Three of a kind
    }
    if pair_count == 2 {
        return (2 << 20) | (pair_rank_hi << 12) | (pair_rank_lo << 8); // Two pair
    }
    if pair_count == 1 {
        return (1 << 20) | (pair_rank_hi << 16); // One pair
    }
    tb // High card
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pair_beats_high_card() {
        // Pair of aces (card 12 = ace of clubs, card 25 = ace of diamonds)
        let pair_hand = evaluate_hand_rank(&[12, 25, 0, 1, 3, 5, 7]);
        // High card king (card 11 = king of clubs, card 23 = queen of diamonds)
        let high_card = evaluate_hand_rank(&[11, 23, 0, 1, 3, 5, 7]);
        assert!(pair_hand > high_card, "Pair should beat high card");
    }

    #[test]
    fn test_flush_beats_straight() {
        // Flush: all clubs (suit 0), ranks 0,2,4,6,8
        let flush = evaluate_hand_rank(&[0, 2, 4, 6, 8, 14, 27]);
        // Straight: ranks 4,5,6,7,8 across suits
        let straight = evaluate_hand_rank(&[4, 18, 6, 20, 8, 14, 27]);
        assert!(flush > straight, "Flush should beat straight");
    }

    #[test]
    fn test_full_house_beats_flush() {
        // Three aces + pair of kings
        let full_house = evaluate_hand_rank(&[12, 25, 38, 11, 24, 0, 1]);
        // Flush: all clubs
        let flush = evaluate_hand_rank(&[0, 2, 4, 6, 8, 14, 27]);
        assert!(full_house > flush, "Full house should beat flush");
    }
}
