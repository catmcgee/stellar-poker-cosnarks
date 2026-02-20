use crate::cards;

/// Derive the final secret deck and salts from all three party contributions.
///
/// - The final deck is obtained by composing three private permutations.
/// - The final salt for each position is the sum of three private salt shares.
pub fn derive_shared_deck_and_salts(
    party0_permutation: [u32; 52],
    party1_permutation: [u32; 52],
    party2_permutation: [u32; 52],
    party0_salts: [Field; 52],
    party1_salts: [Field; 52],
    party2_salts: [Field; 52],
) -> ([Field; 52], [Field; 52]) {
    assert_valid_permutation(party0_permutation);
    assert_valid_permutation(party1_permutation);
    assert_valid_permutation(party2_permutation);

    let mut canonical_deck: [Field; 52] = [0; 52];
    for i in 0..52 {
        canonical_deck[i] = i as Field;
    }

    let deck_after_party0 = apply_permutation(canonical_deck, party0_permutation);
    let deck_after_party1 = apply_permutation(deck_after_party0, party1_permutation);
    let final_deck = apply_permutation(deck_after_party1, party2_permutation);

    let mut final_salts: [Field; 52] = [0; 52];
    for i in 0..52 {
        final_salts[i] = party0_salts[i] + party1_salts[i] + party2_salts[i];
    }

    (final_deck, final_salts)
}

fn assert_valid_permutation(permutation: [u32; 52]) {
    let mut as_fields: [Field; 52] = [0; 52];
    for i in 0..52 {
        assert(permutation[i] < 52, "permutation index out of range");
        as_fields[i] = permutation[i] as Field;
    }
    cards::assert_all_unique(as_fields);
}

fn apply_permutation(deck: [Field; 52], permutation: [u32; 52]) -> [Field; 52] {
    let mut out: [Field; 52] = [0; 52];
    for i in 0..52 {
        out[i] = deck[permutation[i]];
    }
    out
}
