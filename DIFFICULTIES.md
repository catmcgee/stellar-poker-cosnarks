# Difficulties: ZK + MPC on Stellar

Issues encountered building an on-chain poker game with MPC secret sharing and UltraHonk ZK proofs on Stellar/Soroban.

## 1. Transcript Protocol Mismatch (co-noir vs BB Soroban Verifier)

**The biggest issue.** The co-noir MPC prover (TACEO co-snarks) and the Soroban UltraHonk verifier use fundamentally different Fiat-Shamir transcript protocols, making proofs from one unverifiable by the other.

**co-noir prover** (current version):
- Pre-hashes the entire VK (header fields + all 28 commitment points) into a single `vk_hash` field element via `hash_through_transcript`
- Builds the eta challenge hash as: `[vk_hash, ALL_public_inputs (user + pairing accumulator), wire_commitments_raw(x,y)]`
- G1 points encoded as 2 raw field elements (x, y) in the transcript hash

**Soroban verifier** (ported from older BB):
- Hashes VK header fields individually: `[circuit_size, num_public_inputs, pub_inputs_offset]`
- Builds eta hash as: `[circuit_size, num_pi, offset, user_public_inputs, pairing_points, wire_commitments_limb_encoded]`
- G1 points encoded as 4 limb fields (x_lo, x_hi, y_lo, y_hi) in the transcript hash

This produces completely different Fiat-Shamir challenges, causing `Error(Contract, #7)` (VerificationFailed) on every proof submission. The proof itself is mathematically valid (co-noir's native Rust verifier accepts it), but the Soroban verifier reconstructs different challenges and fails.

**Fix applied**: Rewrote the Soroban verifier's `transcript.rs` (~50 lines changed) to match co-noir's protocol: added `compute_vk_hash()` to pre-hash the VK, changed `push_point()` to raw (x,y) encoding, and updated `generate_eta_challenge()` to use the VK hash. The core verification math (sumcheck, shplemini, pairing) was left untouched.

## 2. Proof Format Conversion

co-noir's keccak proofs use a variable-length format with raw G1 point coordinates (2 fields per point), while the Soroban verifier expects a fixed-size format with limb-encoded G1 points (4 fields per point) and padded arrays. Required writing a custom `convert_keccak_proof_to_soroban()` function that:
- Derives `log_n` from proof size
- Limb-encodes all G1 points (splits each 32-byte coordinate into lo136/hi118 halves)
- Pads sumcheck univariates (log_n rounds to 28 rounds)
- Pads Gemini fold commitments and evaluations
- Appends log_n as the final field

## 3. VK Format Incompatibility

Barretenberg outputs VKs in a limb-encoded format (3680 bytes: 3x32 header + 28x128 per-point with 4 limbs each). The Soroban verifier expects a compact format (1824 bytes: 4x8 header + 28x64 raw points). Required a separate Python conversion script (`convert-vk.py`).

Additionally, the `PROOF_FIELDS` constant in the Soroban verifier was hardcoded to 456 but co-noir 0.7.0 with poseidon2 produces 457-field proofs (14624 bytes). Had to update this.

## 4. Entity Count Mismatch

The Soroban verifier originally expected 40 evaluation entities but co-noir produces 41 (28 precomputed + 8 witness + 5 shifted). The extra entity is `Q_POSEIDON2_INTERNAL` which was added in newer UltraHonk versions. Had to update the verifier's entity count and evaluation ordering.

## 5. Soroban Contract Instruction Limits

Soroban contracts have CPU instruction limits. UltraHonk verification involves heavy elliptic curve operations (multi-scalar multiplications, pairings). Need to pass `--instructions 500000000` to avoid hitting limits. This is a concern for mainnet deployment where instruction budgets are enforced.

## 6. Stellar CLI Ergonomics for Complex Arguments

Passing proof bytes and public inputs to Soroban contracts via `stellar contract invoke` is awkward. Proof data (14+ KB hex-encoded), public inputs (arrays of 32-byte hex values), and nested structs all need to be passed as CLI arguments. The hex encoding/decoding adds complexity.

## 7. co-noir Installation and CRS

co-noir requires building from source (`cargo install --git`) and downloading a Common Reference String (CRS/SRS) file. The CRS download isn't documented prominently. Additionally, co-noir uses peer-to-peer TCP between MPC nodes (ports 10000-10002) which requires careful network setup.

## 8. Noir Compiler Version Constraints

Nargo.toml's `compiler_version` field rejects pre-release version constraints. Had to use `>=0.36.0` instead of specifying the exact beta version (1.0.0-beta.13). Also, the circuit compilation produces different artifacts depending on the Noir version, affecting VK compatibility.

## 9. Poseidon2 Hash Compatibility

The Poseidon2 hash used in circuits must exactly match the off-chain implementation. Required porting the exact round constants from Barretenberg's `poseidon2_params.hpp` (t=4, 4+56+4 rounds over BN254). Any mismatch in round constants, MDS matrix, or field arithmetic causes commitment/proof failures.

## 10. No Established ZK Tooling for Stellar

Unlike Ethereum (which has Solidity verifiers, circom/snarkjs, established patterns), Stellar/Soroban has no established ZK verification ecosystem. The Soroban verifier used here was an independent port of BB's verifier, with no community maintenance or version tracking against upstream BB changes. This makes keeping up with prover updates (like the VK hash transcript change) entirely manual.

## 11. Soroban Resource Budgeting Is Moving + Fragile for Large ZK Calls

For large UltraHonk proofs, `stellar contract invoke` resource behavior is sensitive to CLI flag usage and simulation outcomes:
- `--instructions` is deprecated, but older scripts still rely on it.
- Simulation may pass while submission fails with `ResourceLimitExceeded`.
- Costs vary across `commit_deal`, `reveal_board`, and `submit_showdown` because proof/public-input sizes differ by circuit and `log_n`.

Practical impact: coordinator-side invocation needed retry logic with increasing `--instruction-leeway`, not a single fixed budget.

## 12. Off-Chain / On-Chain Table ID Drift Is Easy to Introduce

The coordinator API accepts `{table_id}` while on-chain submission may use an env-configured table id. If these diverge, proof submission fails with phase/table errors even when proofs are valid.

Observed pitfalls:
- `.env.local` exported `TABLE_ID` while coordinator expected `ONCHAIN_TABLE_ID`.
- test script used one table id for coordinator requests and a different hardcoded id for on-chain reads/actions.

Practical impact: perfect MPC/ZK proof generation still produces broken on-chain state transitions unless table-id mapping is explicit and consistent.

## 13. Fail-Open Submission Semantics Hide Real On-Chain Failures

Coordinator originally treated on-chain submission errors as non-fatal warnings and continued game progression off-chain. This is convenient for demos but dangerous for correctness:
- off-chain session progresses to later phases while chain remains behind,
- later calls fail with misleading phase errors,
- root-cause signal (first failed tx) is buried in logs.

Practical impact: when Soroban is configured, submission failures should be considered hard failures for end-to-end correctness.

## 14. Contract Phase Logic Can Be Correct-by-Code but Surprising for Poker UX

The current betting-round completion rule advances phase as soon as all active players' `bet_this_round` matches current max bet. In heads-up, this can transition after the first action in a round, making a "second expected action" invalid (`NotInBettingPhase`).

This is not a cryptography bug, but it complicates integration testing and user-flow assumptions:
- client scripts must re-check phase after each action,
- fixed "two actions per round" scripts can fail even when contract state is internally consistent.

Practical impact: smart-contract game semantics and client/orchestrator assumptions must be validated together, not independently.

## 15. End-to-End Determinism Depends on Coordinating Many Tools, Not Just Proof Math

Even with valid circuits/proofs/verifier:
- local service startup order and process lifetime,
- key/identity/env propagation,
- contract deployment freshness,
- and CLI argument serialization of large blobs
all materially affect success.

Practical impact: Stellar + co-noir integration needs strong operational discipline (fresh deploy scripts, strict env consistency checks, and explicit health/phase gates) to avoid non-cryptographic failures dominating development time.

## 16. Freighter Browser API Drift Breaks Local UX Even When Contracts Are Correct

Freighter integration is version-sensitive:
- some builds expose `window.freighterApi`,
- others rely on the official `@stellar/freighter-api` messaging bridge,
- extension injection can be delayed after page load.

A frontend that only checks one global object can report "wallet not found" even when Freighter is installed, unlocked, and on the correct local network.

Practical impact: wallet connectivity for local Soroban testing should use the official API package first, keep legacy fallbacks, and tolerate delayed injection to avoid false-negative connection failures.

## 17. Message-Signing Standard Mismatch Causes 401s Despite Valid Wallet Connection

Coordinator auth originally verified signatures over raw request message bytes:
- `stellar-poker|address|table_id|action|nonce|timestamp`

Modern Freighter `signMessage` follows SEP-53 semantics:
- signature is over `SHA256("Stellar Signed Message:\\n" + message)`, not raw bytes.

Result: wallet connects successfully, but every authenticated request can fail with `401 Unauthorized` because signatures are valid under SEP-53 but invalid under raw-byte verification.

Practical impact: backend auth verification must support SEP-53 message-hash verification (and optionally legacy raw mode for backward compatibility) to keep browser-wallet UX functional.

## 18. Coordinator Session State Was Volatile While Chain State Persisted

Coordinator kept critical hand state only in memory (`tables` map). After a coordinator restart:
- on-chain table could still be at `Flop/Turn/River`,
- MPC nodes could still have active contribution state,
- but API calls like `request-reveal` failed with `404` because local session was missing.

Practical impact: without recovery, a routine backend restart breaks active hands even though chain + MPC are still recoverable.

Mitigation implemented: lazy session rehydrate from on-chain table state on reveal/showdown/card lookup, including phase/player order/deck root/commitments and deterministic dealt-index reconstruction.

## 19. On-Chain Field Encoding (Hex) vs MPC Input Encoding (Decimal) Mismatch

On-chain table state stores field values (e.g., `deck_root`, `hand_commitments`) as hex strings, while MPC prepare APIs expect decimal field strings for Noir/co-noir inputs.

During session rehydrate this caused split-input failures like:
- `Expected witness values to be integers`
- when passing hex `deck_root` directly into `prepare-reveal`.

Practical impact: any bridge from chain state back into MPC inputs must normalize field encoding (hex -> canonical field element -> decimal) before invoking co-noir.

## 20. Deal Submission Requires `start_hand` Phase Transition

`commit_deal` only succeeds in `GamePhase::Dealing`. After a hand settles, table phase is `Settlement`, so directly submitting a new deal proof fails with:
- `Error(Contract, #21)` (`NotInDealingPhase`)

Practical impact: frontend "DEAL" cannot call proof submission directly; the coordinator must first trigger `start_hand` when table phase is `Waiting` or `Settlement`.

## 21. Auto-Betting Step Was Vulnerable to Transient Stellar CLI Transport Errors

The coordinator auto-submits `player_action` (check/call) to advance betting phases before reveal/showdown. These calls occasionally failed with transient RPC transport errors (e.g., `connection reset by peer`), causing spurious `502` failures during normal game flow.

Practical impact: auto-betting calls need retry logic (like proof submission paths) so transient network failures do not break hand progression.

## 22. Fixed 300s Proof Poll Timeout Was Too Short for Showdown in Practice

Coordinator proof collection used a hardcoded 300-second timeout for all circuits. Showdown MPC proving can exceed this on local hardware/load, causing false failures:
- `proof generation timed out after 300 seconds`
- even though nodes were still actively computing proof.

Practical impact: timeout policy should be circuit-aware; showdown needs a significantly longer ceiling than deal/reveal.

## 23. Seat-Gated Reveal/Showdown API Caused 401s for Legit Operator Flows

Coordinator originally required the caller address to be in `session.player_order` for reveal/showdown endpoints. In practice this blocked local testing/operator flows:
- table/session recovery after restart,
- spectator-triggered continuation,
- and wallets not currently mirrored in in-memory seat mapping.

Practical impact: reveal/showdown control paths should authenticate caller identity but do not need strict seat membership checks (private card access remains protected by `get_player_cards` auth).

## 24. Freighter Signature Format Variance Still Produces 401 in Local Dev

Even after SEP-53 support, browser-wallet auth can still fail locally due version/format drift:
- some Freighter builds return signature payload shapes that do not normalize cleanly to raw 64-byte Ed25519 signatures,
- frontend can report "connected" while coordinator rejects action requests as `401 Unauthorized`,
- this blocks reveal/showdown controls even though wallet + network are otherwise configured.

Practical impact: local dev stacks need a controlled auth bypass mode for iteration. Added `ALLOW_INSECURE_DEV_AUTH=1` path in coordinator/start script to unblock local end-to-end testing; this should stay disabled outside local environments.

## 25. ZK Verifier Rejection Can Deadlock Table Phase Without Fallback Settlement

When on-chain `submit_showdown` fails (e.g., verifier `Error(Contract, #7)`), the hand remains in `Showdown` and cannot progress to a new deal. Without an automated escape hatch:
- frontend loops on showdown retries,
- coordinator session and on-chain phase diverge,
- table appears permanently stuck to players.

Practical impact: coordinator needs explicit fallback handling for committee failure. Added timeout-claim fallback (`claim_timeout`) after showdown submission failure so table can transition to settlement and continue local testing despite verifier instability.

## 26. co-noir MPC Proof Jobs Can Fail with Local TCP Bind Collisions

During reveal proving, co-noir occasionally failed with:
- `while connecting to network`
- `Address already in use (os error 48)` from `mpc-net/src/tcp.rs`

This appeared even with healthy long-running MPC nodes, indicating transient local socket/port collisions during proof orchestration.

Practical impact: some proof attempts fail non-deterministically at the transport layer and require retrying the same API action. This is an operational stability issue around local co-noir networking rather than circuit/verifier correctness.

## 27. Multi-Player Auto-Betting Needs Identity Mapping Beyond Two Wallets

Coordinator auto-advances betting before reveal/showdown by submitting `player_action` as the current on-chain player. Original config only mapped:
- `PLAYER1_ADDRESS` <-> `player1-local`
- `PLAYER2_ADDRESS` <-> `player2-local`

With 3+ seated players, auto-advance failed whenever turn moved to an unmapped seat.

Practical impact: local multi-player flow requires explicit identity mapping for every possible acting seat (e.g., `PLAYER3_ADDRESS/PLAYER3_IDENTITY` ... up to `PLAYER6_*`), not just the first two players.

## 28. One-Step Betting Auto-Advance Works Heads-Up but Is Insufficient for 3+ Players

Initial auto-advance logic submitted only one `Check`/`Call` before reveal/showdown. This often works in heads-up but can leave 3+ tables mid-betting-round, so reveal/showdown calls still fail on phase checks.

Practical impact: coordinator must iterate auto-actions until phase actually changes (with a safety cap) to make multi-player round progression reliable in a reveal-button-driven frontend.

## 29. Generated `.env.local` Had Duplicate Player Address Keys

`deploy-local.sh` initially wrote `PLAYER1_ADDRESS`/`PLAYER2_ADDRESS` twice:
- once in the fixed env template block,
- and again in the per-player loop (`PLAYERn_*` entries).

Values happened to match, but duplicate keys make debugging harder and can mask stale values when scripts evolve.

Practical impact: env generation should emit each key exactly once to avoid silent precedence confusion in shell sourcing and service startup.

## 30. Static `ONCHAIN_TABLE_ID` Override Blocks Multi-Table Routing

Coordinator originally resolved every table operation through a single env override:
- `resolve_onchain_table_id(config, table_id) = ONCHAIN_TABLE_ID || table_id`

When `ONCHAIN_TABLE_ID=0` is set (default local setup), all API calls for table `1`, `2`, ... were silently redirected to table `0`.

Practical impact: create/join/open-table UX appears broken unless coordinator supports true per-request table IDs. Multi-table workflows require using actual `{table_id}` for non-zero table IDs.

## 31. Deal Commitment Count Must Match On-Chain Seated Player Count

`commit_deal` enforces:
- `hand_commitments.len() == table.players.len()`

In lobby mode with pre-seeded on-chain seats, generating a deal proof for only the currently joined wallets caused `WrongCommitmentCount` on-chain failures.

Practical impact: if a table is pre-seeded with N on-chain seats, deal proof generation must still use N players (joined wallets + deterministic placeholders for unclaimed seats), or the contract rejects the hand.

## 32. Browser-Wallet Join UX Conflicts with Contract-Level `require_auth` in Local Dev

`join_table` requires `player.require_auth()` and token transfer from that exact player account. In local coordinator-driven flows this creates friction:
- wallets connected in browser are often not the same pre-seeded local identities,
- coordinator cannot cryptographically join arbitrary wallets on-chain without their signing authority,
- but proof pipeline still needs deterministic on-chain seat count/address set.

Practical impact: practical local UX uses pre-seeded on-chain seats plus a coordinator-side wallet-to-seat lobby mapping. This enables “share table ID / join table” product behavior for testing while preserving on-chain proof constraints.

## 33. Mixed Player-Source Paths (Manual List vs Lobby Seats) Broke Deal Reliability

After adding lobby-based create/join flows, the frontend still had legacy paths that sent an explicit `players` list for heads-up/multi modes. On pre-seeded tables this can diverge from on-chain seat count (N) and produce deals that fail at submission (`WrongCommitmentCount`) even though MPC proving succeeds.

Practical impact: coordinator/frontend must use a single source of truth for seat resolution. For lobby tables, frontend should send an empty `players` list and let coordinator derive all on-chain seats (joined wallets + unclaimed placeholders) deterministically.

## 34. Wallet-Signed API Auth Caused Repeated Freighter Prompts During One Hand

Coordinator auth originally required a fresh signed message for every API mutation (`create`, `join`, `deal`, each `reveal`, `showdown`, etc). In solo flow, post-deal auto-steps trigger additional authenticated calls, which looked like repeated "confirm transaction" prompts in Freighter.

Practical impact: local UX is confusing and high-friction unless frontend leverages insecure-dev auth mode (`ALLOW_INSECURE_DEV_AUTH=1`) and only falls back to signatures when required.

## 35. PixelWorld Music Restarted Across Route Changes

Ambient tracks were created and torn down inside `PixelWorld` component mount/unmount. Navigating between home/table pages recreated audio elements and reset playback.

Practical impact: seamless game ambience requires shared audio state that survives route transitions (singleton/global audio instances), not per-page lifecycle-owned audio objects.

## 36. Soroban CLI Enum Argument Encoding for Payload Variants Is Fragile

For contract enum `Action`, unit variants work as simple strings (`"Call"`), but payload variants must be object JSON with string-encoded i128 (e.g., `{"Bet":"20"}`, `{"Raise":"20"}`). Vector-style encodings (`["Bet",20]`) currently panic in `soroban-spec-tools` in this environment.

Practical impact: coordinator/frontend action serialization must use the object form for `Bet`/`Raise` to avoid CLI parser crashes and ambiguous runtime errors.

## 37. Table Create 502 from Seat Pre-Join Buy-In Mismatch

`/api/tables/create` creates the table and then immediately pre-joins configured local seats. If the coordinator uses a buy-in outside the table config bounds (`InvalidBuyIn`, contract #4) or too high for local token balances, pre-join fails and the endpoint returns `502` even though table creation itself succeeded.

Practical impact: create-table flow must normalize buy-in against the reference table config min/max and use a realistic local default. In this repo we now default to `1_000_000_000` and clamp to config bounds before pre-seeding seats.

## 38. Pre-Seeding All Seats Caused Phantom Opponents and Hidden Auto Buy-Ins

Initial lobby implementation pre-seeded every seat (`max_players`) at table creation time using local identities. This made a newly created table appear to already have opponents and immediately moved token balances for those seats, which looked like implicit buy-ins users did not perform.

Practical impact: seat creation should be demand-driven. Current behavior is now:
- create table seeds zero seats,
- each player must join by signing `join_table` from their own wallet,
- table occupancy comes directly from on-chain seated players.

## 39. Coordinator-Joined Seats Are Not Production-Equivalent Auth

Using coordinator identities to call `join_table`/`player_action` can keep demos moving, but it is not equivalent to production auth:
- contract `require_auth()` is satisfied by coordinator/local identities, not by the browser wallet,
- wallet appears "connected" in UI while the actual on-chain actor is a different account,
- this causes confusing seat ownership, phantom opponents, and surprise chip movements.

Practical impact: production-like testing must use wallet-signed contract transactions for joins and betting actions. Coordinator should not silently impersonate players.

## 40. API Message Signatures and On-Chain Signatures Solve Different Problems

Freighter `signMessage` for coordinator API auth proves HTTP caller identity, but does **not** authorize Soroban contract execution. Only wallet-signed transaction XDR (`signTransaction`) satisfies on-chain `require_auth`.

Practical impact: a stack can pass API auth and still fail to behave like production if state-changing gameplay actions are not submitted as wallet-signed on-chain transactions.

## 41. Chain Config Discovery Is Required for Wallet-Direct Frontends

When frontend submits Soroban tx directly, it needs the exact runtime chain parameters:
- RPC URL,
- network passphrase,
- contract ID.

Hardcoding these in frontend quickly drifts from deployed coordinator/contract state and causes invalid signatures or failed submissions on the wrong network.

Practical impact: expose chain config from backend (`/api/chain-config`) and use it as single source of truth for wallet-signed contract calls.

## 42. Token Denomination Mismatch (Stroops vs UI Chips) Created Buy-In Confusion

On local deployment the poker token is native XLM via SAC, where contract balances are `i128` stroops (1 XLM = 10,000,000 stroops). Frontend initially displayed raw on-chain amounts directly as "CHIPS" (for example `1,000,000,000`) and create-table did not expose explicit buy-in selection.

Practical impact:
- users could not tell how to fund buy-ins or what unit they were approving,
- table creation did not clearly communicate/enforce intended buy-in at UX level,
- wallet join failures looked arbitrary when users expected human-scale chip amounts.

Mitigation in this repo:
- create-table now accepts a user-selected buy-in amount,
- coordinator applies it to table config as fixed min/max buy-in,
- frontend input is now in XLM units and converted to stroops for on-chain calls.

## 43. Dealer Actions Triggered Multiple Wallet Prompts Due Mixed Auth Paths

Frontend dealer flow uses coordinator APIs for deal/reveal/showdown and also fetches private hole cards after deal. With strict API auth enabled, each protected request requires a fresh wallet `signMessage` challenge (`request_deal`, then `get_player_cards`), which users often interpret as duplicate transaction confirmations.

Practical impact: UX appears to "double-sign" a single dealer action even when only one on-chain transaction is actually submitted by committee. Product copy and flow design must distinguish:
- API message-sign auth prompts,
- wallet transaction-sign prompts for actual on-chain `require_auth` actions.

## 44. Manual Street Buttons Create Conflicting Mental Model with Automated Betting Rounds

Showing explicit dealer buttons (`DEAL FLOP`, `DEAL TURN`, `DEAL RIVER`, `SHOWDOWN`) next to betting controls makes users believe dealer progression is a separate manual operator task, not a consequence of betting round completion.

Practical impact: poker UX is clearer when:
- player actions (fold/check/call/bet/raise/all-in) are central and persistent,
- dealer progression is automatic once the round is complete,
- the status line narrates current dealer/proof activity.

## 45. Solo Table Creation Can Fail with 502 When Seeded Local Seats Run Out of Balance

Solo create flow auto-seats deterministic local identities (creator seat + bot seat). After enough tables/hands, these identities can run low on spendable native balance, causing `join_table` token transfer simulation failure (`Error(Contract, #10)`), and the API returns `502`.

Practical impact:
- frontend reports generic create failure even though table creation itself may have succeeded,
- failure is intermittent and worsens over longer test sessions,
- debugging is non-obvious without contract diagnostic logs.

Mitigation in this repo:
- coordinator now auto-top-ups local seat identities via friendbot before join attempts,
- if first join fails with an insufficient-balance signature, it tops up and retries once.
