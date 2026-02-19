# Production Readiness TODO

## Critical

- [ ] **Convert contract panics to proper errors** — Create `#[contracterror]` enum for poker-table (like zk-verifier has). Replace all `panic!()` and `assert!()` with `Result` returns. ~15 `unwrap()` calls on storage reads in `betting.rs`, `lib.rs`, `game.rs`, `pot.rs` need error handling. Currently a bad input crashes the contract and can lock player funds.

- [ ] **Remove plaintext deck from coordinator** — `api.rs` stores the full deck in `TableSession.deck_order` as plaintext. The comment says "never stored in plaintext" but it is. Must only store commitments and Merkle root on the coordinator; actual cards should only exist as secret shares on MPC nodes.

- [ ] **Wallet integration** — Frontend hardcodes `"player_0"` / `"player_1"` (`Table.tsx:19`). Need Freighter wallet connection, real Stellar addresses, and auth signatures on API calls.

- [ ] **Input validation on coordinator API** — `request_deal()` doesn't validate table_id, player count, or game phase. `request_reveal()` accepts arbitrary phase strings. No rate limiting, no replay protection.

- [ ] **Soroban submission atomicity** — If on-chain submission fails, the coordinator still saves the session and returns success. Need either: fail the request, implement rollback, or add a retry queue with reconciliation.

- [ ] **Mock verifier in production path** — `verifier.rs` has a `#[contractimpl]` that always returns `Ok(true)`. This is fine for tests but the mock contract impl ships in the WASM. Gate it behind `#[cfg(test)]` or move to a separate test module.

## Important

- [ ] **Integration tests** — No E2E test for the full flow: create table -> deal -> bet -> reveal -> showdown -> settlement. No test that real proofs pass the zk-verifier. No multi-node MPC coordination test.

- [ ] **`.env.example`** — Document all required env vars: `MPC_NODE_0-2`, `COMMITTEE_SECRET`, `SOROBAN_RPC`, `POKER_TABLE_CONTRACT`, `NETWORK_PASSPHRASE`, `CIRCUIT_DIR`, `NEXT_PUBLIC_COORDINATOR_URL`.

- [ ] **Admin emergency pause** — No way to pause a table if a bug is found. Add `pause_table(admin)` that freezes all actions and `unpause_table(admin)`.

- [ ] **Session ID uniqueness** — `lib.rs:194` sets `session_id = hand_number`, which reuses across tables. Use a hash of `(table_id, hand_number)` or a random nonce.

- [ ] **Committee slash reporter whitelist** — `committee-registry` accepts slash reports from any address (`lib.rs:238`). Should only accept from the poker-table contract.

- [ ] **Proof size validation in coordinator** — zk-verifier checks `PROOF_BYTES` (14,592) but coordinator doesn't validate before submitting. Fail fast instead of wasting a tx.

- [ ] **Public inputs binding** — `commit_deal`/`reveal_board`/`submit_showdown` pass `public_inputs` to the verifier but don't check that those inputs match the on-chain game state (deck_root, commitments, board cards).

- [ ] **Fix Docker builds** — Both Dockerfiles have `cargo build --release 2>/dev/null || true` which silently swallows build failures. Remove `|| true`.

- [ ] **Pin co-noir version** — Dockerfiles install co-noir from `main` branch with no version pin. A breaking upstream change will silently break builds.

- [ ] **CRS download in setup** — `scripts/download-crs.sh` exists but isn't called from `setup.sh` or `docker-compose.yml`. MPC nodes will fail without `crs/bn254_g1.dat`.

- [ ] **Timeout enforcement** — Contract stores `last_action_ledger` and has `claim_timeout()` but there's no automatic trigger or incentive to call it.

- [ ] **Side pot edge cases** — `pot.rs` comment says "simplified for v1". Multi-way all-in with different stack sizes will miscalculate. Add comprehensive tests.

## Nice to Have

- [ ] **Rate limiting** — No per-table or per-IP rate limits on coordinator API. Add tower middleware.

- [ ] **Audit logging** — No persistent record of game actions, proofs submitted, or tx hashes. Add event log for dispute resolution.

- [ ] **Hand evaluation in circuit** — Currently hand ranking runs in Rust (`hand_eval.rs`), not inside the Noir showdown circuit. A malicious coordinator could misrank hands.

- [ ] **Card uniqueness in circuits** — Circuits should assert exactly 52 unique cards. Without this, duplicate cards could be dealt.

- [ ] **Frontend API URL config** — Hardcoded fallback to `localhost:8080`. Add runtime config via env or localStorage.

- [ ] **Deployment automation** — `scripts/deploy.sh` has manual steps for VK registration and committee member setup. Automate with `stellar contract invoke` calls.

- [ ] **Testnet deployment guide** — Docs say "local only". Need guide for testnet account funding, Freighter setup, contract address registry.

- [ ] **zk-verifier WASM build** — Fails due to missing global allocator in `ultrahonk_soroban_verifier` vendor crate. Needs fix in vendor or a custom allocator shim.

- [ ] **Clean up `#[allow(dead_code)]`** — Several files suppress dead code warnings. Remove unused code or use it.
