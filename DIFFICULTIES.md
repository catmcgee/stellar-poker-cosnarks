# Difficulties: ZK + MPC on Stellar

Issues encountered building an on-chain poker game with MPC secret sharing and UltraHonk ZK proofs on Stellar/Soroban.

## 1. Transcript Protocol Mismatch (co-noir vs BB Soroban Verifier)

**The biggest issue.** The co-noir MPC prover (TACEO co-snarks) and the Soroban UltraHonk verifier use fundamentally different Fiat-Shamir transcript protocols, making proofs from one unverifiable by the other.

**co-noir prover** (current version):

- Pre-hashes the entire VK (header fields + all 28 commitment points) into a single `vk_hash` field element via `hash_through_transcript`
- Builds the eta challenge hash as: `[vk_hash, ALL_public_inputs (user + pairing accumulator), wire_commitments_raw(x,y)]`
- G1 points encoded as 2 raw field elements (x, y) in the transcript hash

**Soroban verifier** (uses old version of Barretenberg):

- Hashes VK header fields individually: `[circuit_size, num_public_inputs, pub_inputs_offset]`
- Builds eta hash as: `[circuit_size, num_pi, offset, user_public_inputs, pairing_points, wire_commitments_limb_encoded]`
- G1 points encoded as 4 limb fields (x_lo, x_hi, y_lo, y_hi) in the transcript hash

This produces completely different Fiat-Shamir challenges, causing `Error(Contract, #7)` (VerificationFailed) on every proof submission. The proof itself is mathematically valid (co-noir's native Rust verifier accepts it), but the Soroban verifier reconstructs different challenges and fails.

I rewrote the Soroban verifier's `transcript.rs` (~50 lines changed) to match co-noir's protocol: added `compute_vk_hash()` to pre-hash the VK, changed `push_point()` to raw (x,y) encoding, and updated `generate_eta_challenge()` to use the VK hash. The core verification math (sumcheck, shplemini, pairing) was left untouched.

## 2. co-noir Installation and CRS

co-noir requires building from source (`cargo install --git`) and downloading a Common Reference String (CRS/SRS) file. The CRS download isn't documented prominently. Additionally, co-noir uses peer-to-peer TCP between MPC nodes (ports 10000-10002) which requires careful network setup.

## 3. co-noir MPC Proof Jobs Can Fail with Local TCP Bind Collisions

During reveal proving, co-noir occasionally failed with:

- `while connecting to network`
- `Address already in use (os error 48)` from `mpc-net/src/tcp.rs`

This appeared even with healthy long-running MPC nodes, indicating transient local socket/port collisions during proof orchestration.

## 4. VK Format Incompatibility

Barretenberg outputs VKs in a limb-encoded format (3680 bytes: 3x32 header + 28x128 per-point with 4 limbs each). The Soroban verifier expects a compact format (1824 bytes: 4x8 header + 28x64 raw points). Required a separate Python conversion script (`convert-vk.py`).

Additionally, the `PROOF_FIELDS` constant in the Soroban verifier was hardcoded to 456 but co-noir 0.7.0 with poseidon2 produces 457-field proofs (14624 bytes). Had to update this.

## 5. Entity Count Mismatch

The Soroban verifier originally expected 40 evaluation entities but co-noir produces 41 (28 precomputed + 8 witness + 5 shifted). The extra entity is `Q_POSEIDON2_INTERNAL` which was added in newer UltraHonk versions. Had to update the verifier's entity count and evaluation ordering.

## 6. Soroban Contract Instruction Limits

Soroban contracts have CPU instruction limits. UltraHonk verification involves heavy elliptic curve operations (multi-scalar multiplications, pairings). Need to pass `--instructions 500000000` to avoid hitting limits. This is a concern for mainnet deployment where instruction budgets are enforced.

## 7. Stellar CLI Ergonomics for Complex Arguments

Passing proof bytes and public inputs to Soroban contracts via `stellar contract invoke` is awkward. Proof data (14+ KB hex-encoded), public inputs (arrays of 32-byte hex values), and nested structs all need to be passed as CLI arguments. The hex encoding/decoding adds complexity.

## 10. No Established ZK Tooling for Stellar

Unlike Ethereum (which has Solidity verifiers, circom/snarkjs, established patterns), Stellar/Soroban has no established ZK verification ecosystem. The Soroban verifier used here was an independent port of BB's verifier, with no community maintenance or version tracking against upstream BB changes. This makes keeping up with prover updates (like the VK hash transcript change) entirely manual.

## 11. Soroban Resource Budgeting Is Moving + Fragile for Large ZK Calls

For large UltraHonk proofs, `stellar contract invoke` resource behavior is sensitive to CLI flag usage and simulation outcomes:

- `--instructions` is deprecated, but older scripts still rely on it.
- Simulation may pass while submission fails with `ResourceLimitExceeded`.
- Costs vary across `commit_deal`, `reveal_board`, and `submit_showdown` because proof/public-input sizes differ by circuit and `log_n`.

Practical impact: coordinator-side invocation needed retry logic with increasing `--instruction-leeway`, not a single fixed budget.

## 12. Freighter Browser API Drift Breaks Local UX Even When Contracts Are Correct

Freighter integration is version-sensitive:

- some builds expose `window.freighterApi`,
- others rely on the official `@stellar/freighter-api` messaging bridge,
- extension injection can be delayed after page load.

A frontend that only checks one global object can report "wallet not found" even when Freighter is installed, unlocked, and on the correct local network.

Practical impact: wallet connectivity for local Soroban testing should use the official API package first, keep legacy fallbacks, and tolerate delayed injection to avoid false-negative connection failures.

## 13. Message-Signing Standard Mismatch Causes 401s Despite Valid Wallet Connection

Coordinator auth originally verified signatures over raw request message bytes:

- `stellar-poker|address|table_id|action|nonce|timestamp`

Modern Freighter `signMessage` follows SEP-53 semantics:

- signature is over `SHA256("Stellar Signed Message:\\n" + message)`, not raw bytes.

Result: wallet connects successfully, but every authenticated request can fail with `401 Unauthorized` because signatures are valid under SEP-53 but invalid under raw-byte verification.

Practical impact: backend auth verification must support SEP-53 message-hash verification (and optionally legacy raw mode for backward compatibility) to keep browser-wallet UX functional.

## 14. On-Chain Field Encoding (Hex) vs MPC Input Encoding (Decimal) Mismatch

On-chain table state stores field values (e.g., `deck_root`, `hand_commitments`) as hex strings, while MPC prepare APIs expect decimal field strings for Noir/co-noir inputs.

During session rehydrate this caused split-input failures like:

- `Expected witness values to be integers`
- when passing hex `deck_root` directly into `prepare-reveal`.

Practical impact: any bridge from chain state back into MPC inputs must normalize field encoding (hex -> canonical field element -> decimal) before invoking co-noir.

## 15. Deal Submission Requires `start_hand` Phase Transition

`commit_deal` only succeeds in `GamePhase::Dealing`. After a hand settles, table phase is `Settlement`, so directly submitting a new deal proof fails with:

- `Error(Contract, #21)` (`NotInDealingPhase`)

Practical impact: frontend "DEAL" cannot call proof submission directly; the coordinator must first trigger `start_hand` when table phase is `Waiting` or `Settlement`.

## 16. Auto-Betting Step Was Vulnerable to Transient Stellar CLI Transport Errors

The coordinator auto-submits `player_action` (check/call) to advance betting phases before reveal/showdown. These calls occasionally failed with transient RPC transport errors (e.g., `connection reset by peer`), causing spurious `502` failures during normal game flow.

Practical impact: auto-betting calls need retry logic (like proof submission paths) so transient network failures do not break hand progression.

## 17. Freighter Signature Format Variance Still Produces 401 in Local Dev

Even after SEP-53 support, browser-wallet auth can still fail locally due version/format drift:

- some Freighter builds return signature payload shapes that do not normalize cleanly to raw 64-byte Ed25519 signatures,
- frontend can report "connected" while coordinator rejects action requests as `401 Unauthorized`,
- this blocks reveal/showdown controls even though wallet + network are otherwise configured.

Practical impact: local dev stacks need a controlled auth bypass mode for iteration. Added `ALLOW_INSECURE_DEV_AUTH=1` path in coordinator/start script to unblock local end-to-end testing; this should stay disabled outside local environments.

## 18. ZK Verifier Rejection Can Deadlock Table Phase Without Fallback Settlement

When on-chain `submit_showdown` fails (e.g., verifier `Error(Contract, #7)`), the hand remains in `Showdown` and cannot progress to a new deal. Without an automated escape hatch:

- frontend loops on showdown retries,
- coordinator session and on-chain phase diverge,
- table appears permanently stuck to players.

Practical impact: coordinator needs explicit fallback handling for committee failure. Added timeout-claim fallback (`claim_timeout`) after showdown submission failure so table can transition to settlement and continue local testing despite verifier instability.

## 19. Deal Commitment Count Must Match On-Chain Seated Player Count

`commit_deal` enforces:

- `hand_commitments.len() == table.players.len()`

In lobby mode with pre-seeded on-chain seats, generating a deal proof for only the currently joined wallets caused `WrongCommitmentCount` on-chain failures.

Practical impact: if a table is pre-seeded with N on-chain seats, deal proof generation must still use N players (joined wallets + deterministic placeholders for unclaimed seats), or the contract rejects the hand.

## 20. Browser-Wallet Join UX Conflicts with Contract-Level `require_auth` in Local Dev

`join_table` requires `player.require_auth()` and token transfer from that exact player account. In local coordinator-driven flows this creates friction:

- wallets connected in browser are often not the same pre-seeded local identities,
- coordinator cannot cryptographically join arbitrary wallets on-chain without their signing authority,
- but proof pipeline still needs deterministic on-chain seat count/address set.

Practical impact: practical local UX uses pre-seeded on-chain seats plus a coordinator-side wallet-to-seat lobby mapping. This enables "share table ID / join table" product behavior for testing while preserving on-chain proof constraints.

## 21. Soroban CLI Enum Argument Encoding for Payload Variants Is Fragile

For contract enum `Action`, unit variants work as simple strings (`"Call"`), but payload variants must be object JSON with string-encoded i128 (e.g., `{"Bet":"20"}`, `{"Raise":"20"}`). Vector-style encodings (`["Bet",20]`) currently panic in `soroban-spec-tools` in this environment.

Practical impact: coordinator/frontend action serialization must use the object form for `Bet`/`Raise` to avoid CLI parser crashes and ambiguous runtime errors.

## 22. API Message Signatures and On-Chain Signatures Solve Different Problems

Freighter `signMessage` for coordinator API auth proves HTTP caller identity, but does **not** authorize Soroban contract execution. Only wallet-signed transaction XDR (`signTransaction`) satisfies on-chain `require_auth`.

Practical impact: a stack can pass API auth and still fail to behave like production if state-changing gameplay actions are not submitted as wallet-signed on-chain transactions.
