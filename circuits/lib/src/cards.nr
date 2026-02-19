// Card encoding for Stellar Poker.
// Card value = suit * 13 + rank
// suit: 0=Clubs, 1=Diamonds, 2=Hearts, 3=Spades
// rank: 0=2, 1=3, ..., 8=10, 9=J, 10=Q, 11=K, 12=A

pub global DECK_SIZE: u32 = 52;
pub global NUM_SUITS: u32 = 4;
pub global NUM_RANKS: u32 = 13;

// Validate that a card value is in range [0, 51].
pub fn assert_valid_card(card: Field) {
    let card_u32 = card as u32;
    assert(card_u32 < DECK_SIZE, "invalid card value");
}

// Extract suit from card value.
pub fn suit(card: Field) -> u32 {
    (card as u32) / NUM_RANKS
}

// Extract rank from card value.
pub fn rank(card: Field) -> u32 {
    (card as u32) % NUM_RANKS
}

// Verify that a set of cards are all unique (no duplicates).
pub fn assert_all_unique<let N: u32>(cards: [Field; N]) {
    for i in 0..N {
        for j in (i + 1)..N {
            assert(cards[i] != cards[j], "duplicate card");
        }
    }
}

// Verify that a set of cards forms a valid deck (all unique, all in range).
pub fn assert_valid_deck(deck: [Field; 52]) {
    for i in 0..52 {
        assert_valid_card(deck[i]);
    }
    assert_all_unique(deck);
}

// Hand ranking for showdown verification.
// Returns a score where higher = better hand.
//
// Input: 7 cards (2 hole + 5 board), each as u32 value [0, 51].
// Output: score (u32) where higher = better.
pub fn evaluate_hand_rank(cards: [u32; 7]) -> u32 {
    let mut best_score: u32 = 0;

    // Check all C(7,5) = 21 combinations by choosing which 2 to skip
    for skip1 in 0..7 {
        for skip2 in (skip1 + 1)..7 {
            let mut hand: [u32; 5] = [0; 5];
            let mut idx: u32 = 0;
            for k in 0..7 {
                if (k != skip1) & (k != skip2) {
                    hand[idx] = cards[k];
                    idx += 1;
                }
            }
            let score = score_five(hand);
            if score > best_score {
                best_score = score;
            }
        }
    }

    best_score
}

// Score exactly 5 cards. Returns category << 20 | tiebreaker.
fn score_five(cards: [u32; 5]) -> u32 {
    let mut ranks: [u32; 5] = [0; 5];
    let mut suits: [u32; 5] = [0; 5];
    for i in 0..5 {
        ranks[i] = cards[i] % NUM_RANKS;
        suits[i] = cards[i] / NUM_RANKS;
    }

    // Sort ranks descending (bubble sort for 5 elements)
    for i in 0..4 {
        for j in 0..(4 - i) {
            if ranks[j] < ranks[j + 1] {
                let tmp = ranks[j];
                ranks[j] = ranks[j + 1];
                ranks[j + 1] = tmp;
            }
        }
    }

    let is_flush = (suits[0] == suits[1])
        & (suits[1] == suits[2])
        & (suits[2] == suits[3])
        & (suits[3] == suits[4]);

    let is_straight = (ranks[0] == ranks[1] + 1)
        & (ranks[1] == ranks[2] + 1)
        & (ranks[2] == ranks[3] + 1)
        & (ranks[3] == ranks[4] + 1);

    let is_wheel = (ranks[0] == 12)
        & (ranks[1] == 3)
        & (ranks[2] == 2)
        & (ranks[3] == 1)
        & (ranks[4] == 0);

    // Count frequencies
    let mut freq: [u32; 13] = [0; 13];
    for i in 0..5 {
        freq[ranks[i]] += 1;
    }

    let mut has_four = false;
    let mut has_three = false;
    let mut pair_count: u32 = 0;
    let mut four_rank: u32 = 0;
    let mut three_rank: u32 = 0;
    let mut pair_rank_hi: u32 = 0;
    let mut pair_rank_lo: u32 = 0;

    for r_inv in 0..13 {
        let r = 12 - r_inv;
        if freq[r] == 4 {
            has_four = true;
            four_rank = r;
        } else if freq[r] == 3 {
            has_three = true;
            three_rank = r;
        } else if freq[r] == 2 {
            if pair_count == 0 {
                pair_rank_hi = r;
            } else {
                pair_rank_lo = r;
            }
            pair_count += 1;
        }
    }

    let tb = (ranks[0] << 16) | (ranks[1] << 12) | (ranks[2] << 8) | (ranks[3] << 4) | ranks[4];

    // Use conditional accumulation instead of early returns
    let mut score: u32 = 0;
    let mut categorized = false;

    // Royal/straight flush
    if !categorized & is_flush & is_straight & (ranks[0] == 12) {
        score = (9 << 20) | tb;
        categorized = true;
    }
    if !categorized & is_flush & (is_straight | is_wheel) {
        let high = if is_wheel { 3 << 16 } else { tb };
        score = (8 << 20) | high;
        categorized = true;
    }
    if !categorized & has_four {
        score = (7 << 20) | (four_rank << 16);
        categorized = true;
    }
    if !categorized & has_three & (pair_count >= 1) {
        score = (6 << 20) | (three_rank << 8) | pair_rank_hi;
        categorized = true;
    }
    if !categorized & is_flush {
        score = (5 << 20) | tb;
        categorized = true;
    }
    if !categorized & (is_straight | is_wheel) {
        let high = if is_wheel { 3 << 16 } else { ranks[0] << 16 };
        score = (4 << 20) | high;
        categorized = true;
    }
    if !categorized & has_three {
        score = (3 << 20) | (three_rank << 16);
        categorized = true;
    }
    if !categorized & (pair_count == 2) {
        score = (2 << 20) | (pair_rank_hi << 12) | (pair_rank_lo << 8);
        categorized = true;
    }
    if !categorized & (pair_count == 1) {
        score = (1 << 20) | (pair_rank_hi << 16);
        categorized = true;
    }
    if !categorized {
        score = tb;
    }

    score
}

#[test]
fn test_card_encoding() {
    assert(suit(0) == 0);
    assert(rank(0) == 0);
    assert(suit(51) == 3);
    assert(rank(51) == 12);
    assert(suit(37) == 2);
    assert(rank(37) == 11);
}

#[test]
fn test_valid_card() {
    assert_valid_card(0);
    assert_valid_card(51);
    assert_valid_card(25);
}

#[test]
fn test_hand_ranking_pair_beats_high_card() {
    // Pair of aces vs high card king
    let pair_hand = evaluate_hand_rank([12, 25, 0, 1, 3, 5, 7]);
    let high_card = evaluate_hand_rank([11, 23, 0, 1, 3, 5, 7]);
    assert(pair_hand > high_card);
}
