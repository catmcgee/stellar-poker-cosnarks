# Stellar Poker Protocol Specification

## Overview

Stellar Poker implements a trustless Texas Hold'em poker protocol on Stellar's Soroban smart contract platform. Card privacy is achieved through Multi-Party Computation (MPC) using TACEO's coNoir framework, while game integrity is verified through UltraHonk zero-knowledge proofs.

## Actors

1. **Players**: Participants at a poker table. Each player has a Stellar account (keypair) and deposits tokens (buy-in) into the PokerTable contract.

2. **MPC Committee**: A set of 3 nodes running TACEO coNoir. They collectively hold the secret deck state via REP3 (Replicated 3-party) secret sharing. No single node can reconstruct the deck.

3. **Coordinator**: An off-chain service that orchestrates MPC sessions. It does NOT see private data -- it only routes messages between nodes and the blockchain.

4. **Soroban Contracts**: On-chain source of truth for game state, pot management, and proof verification.

## Protocol Phases

### Phase 1: Table Setup

1. Admin calls `PokerTable::create_table(config)` with:
   - Token address (payment asset)
   - Min/max buy-in
   - Small/big blind amounts
   - Max players (2-6)
   - Timeout in ledger sequences
   - Committee address

2. Players call `PokerTable::join_table(table_id, buy_in)`:
   - Buy-in tokens transferred to contract
   - Player seated at next available position

### Phase 2: Hand Start

1. Any player (or the coordinator) calls `PokerTable::start_hand(table_id)`
2. Contract transitions to `Dealing` phase
3. Coordinator detects the event and initiates MPC deal

### Phase 3: MPC Deal (Off-chain)

1. **Shuffle**: Each MPC node generates random shares. Via coNoir's REP3 protocol, these combine into a secret-shared permutation of 52 cards. No node sees the permutation.

2. **Commitment**: Inside MPC, each card gets a random salt. The commitment for card i is:
   ```
   commit_i = Poseidon2(deck[i], salt[i])
   ```
   These commitments are computed on secret shares -- the nodes never see plaintext values.

3. **Merkle Tree**: A Merkle tree (depth 6, 64 leaves padded from 52) is built over commitments:
   ```
   deck_root = MerkleRoot(commit_0, commit_1, ..., commit_51, 0, ..., 0)
   ```

4. **Deal Proof**: The MPC committee generates a `deal_valid` proof (UltraHonk):
   - Private witness: deck[52], salts[52] (secret-shared)
   - Public inputs: deck_root, hand_commitments[num_players], dealt_card_indices
   - Proves: valid deck, correct Merkle root, hand commitments match

5. **Submit On-chain**: Coordinator calls `PokerTable::commit_deal(...)` with deck_root, hand_commitments, dealt_indices, and the proof.

6. **Private Delivery**: Each player's hole cards are revealed only to them via threshold decryption from the MPC nodes (2-of-3 needed).

### Phase 4: Betting Rounds

Standard Texas Hold'em betting:
- **Preflop**: Left of big blind acts first
- **Flop/Turn/River**: Left of dealer acts first

Each player calls `PokerTable::player_action(table_id, action)`:
- `Fold`: Player forfeits hand
- `Check`: Pass (only if no outstanding bet)
- `Call`: Match the current bet
- `Bet(amount)`: Open betting
- `Raise(amount)`: Increase the bet
- `AllIn`: Bet entire stack

The contract validates:
- It is the player's turn (`current_turn`)
- The action is legal for the current state
- Bet amounts are within valid ranges

### Phase 5: Board Reveal

After each betting round:
1. Contract transitions to `DealingFlop/Turn/River`
2. Coordinator triggers MPC reveal:
   - Committee generates `reveal_board_valid` proof
   - Private: deck[52], salts[52]
   - Public: deck_root, revealed_cards, revealed_indices, previously_used
   - Proves: cards match committed deck, no reuse
3. Coordinator calls `PokerTable::reveal_board(...)` with cards, proof
4. Contract advances to next betting phase

### Phase 6: Showdown

1. After river betting, contract transitions to `Showdown`
2. Committee generates `showdown_valid` proof:
   - Reveals all remaining players' hole cards
   - Proves hand evaluation is correct
   - Proves declared winner has the best hand
3. Coordinator calls `PokerTable::submit_showdown(...)`
4. Contract evaluates hands and distributes pot

### Phase 7: Settlement

- Winner receives pot (minus any side pots)
- Side pots are distributed to eligible players based on all-in amounts
- Players can leave or continue to next hand

## Commitment Scheme

### Card Commitment
```
commit(card, salt) = Poseidon2([card, salt, 0, 0], message_length=4)[0]
```

Using Poseidon2 over the BN254 scalar field, with a 4-element state (native state size for BN254 Poseidon2).

### Hand Commitment
```
hand_commit(card1, salt1, card2, salt2) = Poseidon2([
    commit(card1, salt1), commit(card2, salt2), 0, 0
], message_length=4)[0]
```

### Merkle Tree
- Binary tree of depth 6 (64 leaves)
- 52 card commitments + 12 zero-padding
- Internal nodes: `Poseidon2([left, right, 0, 0], 4)[0]`

## Timeout Mechanism

Every action updates `last_action_ledger`. If `timeout_ledgers` pass without an action:

- **Player timeout** (during betting): Anyone can call `claim_timeout`. The stalling player auto-folds. If only one player remains, they win the pot.

- **Committee timeout** (during dealing/reveal/showdown): Anyone can call `claim_timeout`. The hand enters `Dispute` phase. All players receive an emergency refund (pot split equally among non-folded players). The committee can be reported for slashing.

## Security Properties

1. **Card Privacy**: With honest majority (2-of-3 MPC nodes), no party learns any card except those revealed to them. Even the coordinator cannot see cards.

2. **Shuffle Fairness**: The shuffle is performed inside MPC. Each node contributes randomness. The final permutation is uniformly random as long as at least one node is honest.

3. **Proof Soundness**: UltraHonk proofs are computationally sound under standard assumptions (knowledge of exponent over BN254). A cheating committee cannot forge a valid proof.

4. **On-chain Finality**: The Soroban contract is the single source of truth. It will not advance the game state without valid proofs.

5. **Liveness**: Timeouts ensure the game cannot be stalled indefinitely. Players and the committee have bounded time to act.

6. **Economic Security**: Committee members stake tokens. Repeated misbehavior triggers slashing (50% after 3 reports), creating an economic disincentive for cheating.
