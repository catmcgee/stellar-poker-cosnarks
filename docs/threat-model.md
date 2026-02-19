# Stellar Poker Threat Model

## Trust Assumptions

| Entity | Trust Level | Assumption |
|--------|-------------|------------|
| Soroban VM | Fully trusted | Correct execution of smart contracts |
| Protocol 25 BN254 ops | Fully trusted | Correct implementation of cryptographic primitives |
| MPC Committee (3 nodes) | Honest majority (2/3) | At most 1 node is malicious |
| Coordinator | Untrusted for privacy | May observe message patterns but not card values |
| Players | Untrusted | May deviate from protocol arbitrarily |
| Network | Untrusted | Messages may be delayed, reordered, or dropped |

## Threat Categories

### T1: Card Privacy Breach

**Threat**: An adversary learns a player's hole cards before showdown.

**Mitigations**:
- REP3 secret sharing: Each node holds 2-of-3 shares. Reconstructing a secret requires all 3 shares (from 2 distinct nodes).
- With 1 corrupted node: Privacy holds. The corrupted node sees 2 shares but cannot reconstruct without the third.
- With 2 corrupted nodes: They CAN reconstruct secrets. Mitigation: audit logs, on-chain slashing, reputation systems.
- Card delivery uses threshold decryption (2-of-3), ensuring the coordinator alone cannot intercept.

**Residual risk**: 2-of-3 collusion breaks privacy. This is inherent to the 3-party MPC model.

### T2: Rigged Shuffle

**Threat**: The committee produces a non-random shuffle to benefit a specific player.

**Mitigations**:
- Each node contributes independent randomness to the shuffle. The final permutation is uniformly random if even 1 node is honest.
- The `deal_valid` proof verifies the deck is a valid permutation of 52 cards. A rigged deck that duplicates or omits cards will be rejected.
- Poseidon2 commitments bind the shuffle at deal time. The committee cannot change cards after committing.

**Residual risk**: With 2+ colluding nodes, they could bias which cards go to which positions. The ZK proof ensures it is still a valid 52-card deck, but the distribution could be non-random.

### T3: Proof Forgery

**Threat**: The committee submits a fake proof that the contract accepts.

**Mitigations**:
- UltraHonk is computationally sound under the discrete log assumption over BN254.
- Verification uses Soroban's native BN254 host functions (not WASM), which are audited implementations.
- Proof verification is deterministic: same proof + public inputs always produces the same accept/reject.

**Residual risk**: Theoretical: a quantum computer could break BN254 in the far future. Practical: essentially zero with current technology.

### T4: Front-Running / MEV

**Threat**: A validator or observer sees a player's action before it is finalized and acts on this information.

**Mitigations**:
- Soroban transactions are atomic. Once submitted, actions execute in a single ledger.
- Player actions (fold/check/call/bet) do not reveal hidden information.
- The critical hidden information (hole cards) is never on-chain in plaintext.

**Residual risk**: An adversary controlling a validator could potentially reorder transactions within a ledger, but poker actions are sequential (one player at a time), limiting MEV opportunities.

### T5: Denial of Service

**Threat**: A player or the committee stalls the game indefinitely.

**Mitigations**:
- Timeout mechanism: `timeout_ledgers` parameter enforces bounded waiting.
- Player timeout: Auto-fold after timeout. If only 1 player remains, they win.
- Committee timeout: Dispute phase + emergency refund. Committee gets reported for slashing.
- Any third party can trigger `claim_timeout`.

**Residual risk**: Brief delays up to the timeout period are possible.

### T6: Economic Attacks

**Threat**: A player exploits the pot/betting logic for profit.

**Mitigations**:
- All bet validation is on-chain: the contract checks legal actions, amounts, and turn order.
- Buy-in limits prevent excessive exposure.
- Side pot calculation handles all-in scenarios correctly.
- Token transfers use Soroban's built-in token interface with proper authorization.

**Residual risk**: Standard poker-level economic risks (bluffing, pot odds calculation) are inherent to the game.

### T7: Coordinator Compromise

**Threat**: The coordinator is compromised and acts maliciously.

**Mitigations**:
- The coordinator cannot see card values (they exist only as MPC shares).
- The coordinator cannot forge proofs (no private keys for MPC nodes).
- The coordinator cannot modify game state (only the contract can).
- A compromised coordinator can at worst deny service (triggering committee timeout).

**Residual risk**: Availability depends on the coordinator. Future work: decentralize the coordinator role.

### T8: Smart Contract Bugs

**Threat**: A bug in the Soroban contracts allows fund theft or state corruption.

**Mitigations**:
- Contracts use require_auth() for all privileged actions.
- State transitions are explicitly validated (phase checks).
- Token transfers use the standard Soroban token interface.
- Emergency refund mechanism protects player funds if the protocol halts.

**Residual risk**: Undetected logic bugs. Mitigation: formal verification, extensive testing, audits.

## Attack Surface Summary

| Vector | Severity | Likelihood | Status |
|--------|----------|------------|--------|
| 2-of-3 MPC collusion | High | Low | Accepted (inherent to 3-party MPC) |
| Proof forgery | Critical | Negligible | Mitigated by UltraHonk soundness |
| Contract bugs | High | Medium | Mitigated by testing + audit |
| DoS/stalling | Medium | Medium | Mitigated by timeouts |
| Coordinator compromise | Medium | Low | Mitigated by MPC architecture |
| Front-running | Low | Low | Mitigated by sequential actions |

## Future Improvements

1. **Larger committee (5-of-9)**: Increases collusion threshold
2. **Verifiable shuffle**: Add Bayer-Groth shuffle proof for public verifiability
3. **Decentralized coordinator**: Remove single point of failure
4. **Commit-reveal for actions**: Prevent action front-running entirely
5. **Formal verification**: Prove contract correctness with mathematical proofs
