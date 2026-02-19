#![no_std]

use soroban_sdk::contracttype;

/// Card encoding: suit * 13 + rank
/// suit: 0=Clubs, 1=Diamonds, 2=Hearts, 3=Spades
/// rank: 0=2, 1=3, ..., 8=10, 9=J, 10=Q, 11=K, 12=A
pub const DECK_SIZE: u32 = 52;
pub const NUM_SUITS: u32 = 4;
pub const NUM_RANKS: u32 = 13;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Card {
    pub value: u32, // 0-51
}

impl Card {
    pub fn new(suit: u32, rank: u32) -> Self {
        assert!(suit < NUM_SUITS, "invalid suit");
        assert!(rank < NUM_RANKS, "invalid rank");
        Card {
            value: suit * NUM_RANKS + rank,
        }
    }

    pub fn suit(&self) -> u32 {
        self.value / NUM_RANKS
    }

    pub fn rank(&self) -> u32 {
        self.value % NUM_RANKS
    }

    pub fn is_valid(&self) -> bool {
        self.value < DECK_SIZE
    }
}

/// Hand ranking categories (higher = better)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum HandCategory {
    HighCard = 0,
    OnePair = 1,
    TwoPair = 2,
    ThreeOfAKind = 3,
    Straight = 4,
    Flush = 5,
    FullHouse = 6,
    FourOfAKind = 7,
    StraightFlush = 8,
    RoyalFlush = 9,
}

/// A hand ranking that can be compared. Higher value = better hand.
/// Format: category (top 4 bits) | tiebreaker (bottom 28 bits)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandRank {
    pub score: u32,
}

impl HandRank {
    pub fn new(category: u32, tiebreaker: u32) -> Self {
        HandRank {
            score: (category << 28) | (tiebreaker & 0x0FFF_FFFF),
        }
    }

    pub fn category(&self) -> u32 {
        self.score >> 28
    }

    pub fn beats(&self, other: &HandRank) -> bool {
        self.score > other.score
    }
}

/// Evaluate the best 5-card hand from 7 cards (2 hole + 5 board).
/// Returns a HandRank that can be compared to determine winner.
///
/// Cards are passed as an array of 7 card values (0-51).
pub fn evaluate_hand(cards: &[u32; 7]) -> HandRank {
    let mut best_score: u32 = 0;

    // Check all C(7,5) = 21 combinations
    for i in 0..7 {
        for j in (i + 1)..7 {
            // Skip cards at indices i and j (use the other 5)
            let mut hand = [0u32; 5];
            let mut idx = 0;
            for k in 0..7 {
                if k != i && k != j {
                    hand[idx] = cards[k];
                    idx += 1;
                }
            }
            let rank = evaluate_five(&hand);
            if rank.score > best_score {
                best_score = rank.score;
            }
        }
    }

    HandRank { score: best_score }
}

/// Evaluate exactly 5 cards.
fn evaluate_five(cards: &[u32; 5]) -> HandRank {
    let mut ranks = [0u32; 5];
    let mut suits = [0u32; 5];
    for i in 0..5 {
        ranks[i] = cards[i] % NUM_RANKS;
        suits[i] = cards[i] / NUM_RANKS;
    }

    // Sort ranks descending
    sort_desc(&mut ranks);

    let is_flush = suits[0] == suits[1]
        && suits[1] == suits[2]
        && suits[2] == suits[3]
        && suits[3] == suits[4];

    let is_straight = is_straight_hand(&ranks);

    // Also check A-2-3-4-5 (wheel)
    let is_wheel = ranks[0] == 12
        && ranks[1] == 3
        && ranks[2] == 2
        && ranks[3] == 1
        && ranks[4] == 0;

    // Count rank frequencies
    let mut freq = [0u32; NUM_RANKS as usize];
    for &r in ranks.iter() {
        freq[r as usize] += 1;
    }

    // Find groups
    let mut quads = 0u32;
    let mut trips = 0u32;
    let mut pairs = 0u32;
    let mut quad_rank = 0u32;
    let mut trip_rank = 0u32;
    let mut pair_ranks = [0u32; 2];

    for r in (0..NUM_RANKS).rev() {
        match freq[r as usize] {
            4 => {
                quads += 1;
                quad_rank = r;
            }
            3 => {
                trips += 1;
                trip_rank = r;
            }
            2 => {
                if pairs < 2 {
                    pair_ranks[pairs as usize] = r;
                }
                pairs += 1;
            }
            _ => {}
        }
    }

    if is_flush && is_straight {
        if ranks[0] == 12 && ranks[1] == 11 {
            // Royal flush (A-K-Q-J-10)
            return HandRank::new(9, ranks[0]);
        }
        return HandRank::new(8, if is_wheel { 3 } else { ranks[0] });
    }

    if is_flush && is_wheel {
        return HandRank::new(8, 3); // Straight flush, 5-high
    }

    if quads == 1 {
        let kicker = ranks.iter().find(|&&r| r != quad_rank).copied().unwrap_or(0);
        return HandRank::new(7, (quad_rank << 4) | kicker);
    }

    if trips == 1 && pairs >= 1 {
        return HandRank::new(6, (trip_rank << 4) | pair_ranks[0]);
    }

    if is_flush {
        let tb = (ranks[0] << 16) | (ranks[1] << 12) | (ranks[2] << 8) | (ranks[3] << 4) | ranks[4];
        return HandRank::new(5, tb);
    }

    if is_straight || is_wheel {
        return HandRank::new(4, if is_wheel { 3 } else { ranks[0] });
    }

    if trips == 1 {
        let mut kickers = [0u32; 2];
        let mut ki = 0;
        for &r in ranks.iter() {
            if r != trip_rank && ki < 2 {
                kickers[ki] = r;
                ki += 1;
            }
        }
        return HandRank::new(3, (trip_rank << 8) | (kickers[0] << 4) | kickers[1]);
    }

    if pairs == 2 {
        let high_pair = if pair_ranks[0] > pair_ranks[1] { pair_ranks[0] } else { pair_ranks[1] };
        let low_pair = if pair_ranks[0] > pair_ranks[1] { pair_ranks[1] } else { pair_ranks[0] };
        let kicker = ranks.iter().find(|&&r| r != high_pair && r != low_pair).copied().unwrap_or(0);
        return HandRank::new(2, (high_pair << 8) | (low_pair << 4) | kicker);
    }

    if pairs == 1 {
        let pr = pair_ranks[0];
        let mut kickers = [0u32; 3];
        let mut ki = 0;
        for &r in ranks.iter() {
            if r != pr && ki < 3 {
                kickers[ki] = r;
                ki += 1;
            }
        }
        return HandRank::new(1, (pr << 12) | (kickers[0] << 8) | (kickers[1] << 4) | kickers[2]);
    }

    // High card
    let tb = (ranks[0] << 16) | (ranks[1] << 12) | (ranks[2] << 8) | (ranks[3] << 4) | ranks[4];
    HandRank::new(0, tb)
}

fn is_straight_hand(sorted_ranks: &[u32; 5]) -> bool {
    sorted_ranks[0] == sorted_ranks[1] + 1
        && sorted_ranks[1] == sorted_ranks[2] + 1
        && sorted_ranks[2] == sorted_ranks[3] + 1
        && sorted_ranks[3] == sorted_ranks[4] + 1
}

fn sort_desc(arr: &mut [u32; 5]) {
    // Simple insertion sort for 5 elements
    for i in 1..5 {
        let mut j = i;
        while j > 0 && arr[j] > arr[j - 1] {
            arr.swap(j, j - 1);
            j -= 1;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_card_encoding() {
        let card = Card::new(0, 0); // 2 of clubs
        assert_eq!(card.value, 0);
        assert_eq!(card.suit(), 0);
        assert_eq!(card.rank(), 0);

        let card = Card::new(3, 12); // Ace of spades
        assert_eq!(card.value, 51);
        assert_eq!(card.suit(), 3);
        assert_eq!(card.rank(), 12);
    }

    #[test]
    fn test_royal_flush_beats_straight_flush() {
        // Royal flush: 10♣ J♣ Q♣ K♣ A♣ + 2♦ 3♦
        let royal = evaluate_hand(&[8, 9, 10, 11, 12, 13, 14]);
        // Straight flush: 5♣ 6♣ 7♣ 8♣ 9♣ + 2♦ 3♦
        let sf = evaluate_hand(&[3, 4, 5, 6, 7, 13, 14]);
        assert!(royal.beats(&sf));
    }

    #[test]
    fn test_four_of_a_kind_beats_full_house() {
        // Four 2s: 2♣ 2♦ 2♥ 2♠ + K♣ Q♣ J♣
        let quads = evaluate_hand(&[0, 13, 26, 39, 11, 10, 9]);
        // Full house: 3♣ 3♦ 3♥ + K♣ K♦ + Q♣ J♣
        let fh = evaluate_hand(&[1, 14, 27, 11, 24, 10, 9]);
        assert!(quads.beats(&fh));
    }

    #[test]
    fn test_flush_beats_straight() {
        // Flush: 2♣ 4♣ 6♣ 8♣ K♣ + 2♦ 3♦
        let flush = evaluate_hand(&[0, 2, 4, 6, 11, 13, 14]);
        // Straight: 5♣ 6♦ 7♥ 8♠ 9♣ + 2♦ 3♦
        let straight = evaluate_hand(&[3, 17, 31, 45, 7, 13, 14]);
        assert!(flush.beats(&straight));
    }

    #[test]
    fn test_pair_beats_high_card() {
        // Pair: 2♣ 2♦ + 5♣ 7♣ 9♣ K♣ A♣
        let pair = evaluate_hand(&[0, 13, 3, 5, 7, 11, 12]);
        // High card: A♣ K♦ Q♥ J♠ 9♣ + 2♦ 3♦
        let high = evaluate_hand(&[12, 24, 36, 48, 7, 13, 14]);
        assert!(pair.beats(&high));
    }

    #[test]
    fn test_wheel_straight() {
        // A-2-3-4-5 (wheel): A♣ 2♦ 3♥ 4♠ 5♣ + K♦ Q♦
        let wheel = evaluate_hand(&[12, 13, 27, 41, 3, 24, 23]);
        assert_eq!(wheel.category(), 4); // Straight
    }
}
