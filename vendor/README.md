# vendor/

Vendored dependencies that are not available on crates.io.

## ultrahonk-rust-verifier/

Pure Rust implementation of the UltraHonk proof verification algorithm over the BN254 curve. Originally from [yugocabrio/ultrahonk-rust-verifier](https://github.com/yugocabrio/ultrahonk-rust-verifier), modified to work with [TACEO's coNoir](https://github.com/TaceoLabs/co-snarks) MPC proving system.

Our `zk-verifier` contract depends on the `ultrahonk-soroban-verifier` sub-crate inside this directory. It provides the core `UltraHonkVerifier` used to verify Noir circuit proofs (deal, reveal, showdown) on-chain via Soroban's native BN254 host functions (Protocol 25).

You can read some more details of how it was changed inside the file by reading comments.
It does not change verification logic and is still sound.
