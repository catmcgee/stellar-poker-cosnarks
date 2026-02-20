//! Soroban on-chain proof submission via `stellar contract invoke`.
//!
//! Shells out to the Stellar CLI to submit proofs and game state to
//! the on-chain poker-table contract. Uses the same `tokio::process::Command`
//! pattern as `mpc.rs` for co-noir subprocess execution.

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

/// Configuration for Soroban interactions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SorobanConfig {
    pub rpc_url: String,
    pub secret_key: String,
    pub poker_table_contract: String,
    pub network_passphrase: String,
}

impl SorobanConfig {
    pub fn from_env() -> Self {
        Self {
            rpc_url: std::env::var("SOROBAN_RPC")
                .unwrap_or_else(|_| "http://localhost:8000/soroban/rpc".to_string()),
            secret_key: std::env::var("COMMITTEE_SECRET")
                .unwrap_or_else(|_| "test_secret".to_string()),
            poker_table_contract: std::env::var("POKER_TABLE_CONTRACT")
                .unwrap_or_else(|_| String::new()),
            network_passphrase: std::env::var("NETWORK_PASSPHRASE")
                .unwrap_or_else(|_| "Test SDF Network ; September 2015".to_string()),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.poker_table_contract.is_empty() && self.secret_key != "test_secret"
    }

    /// Derive the Stellar public address (G...) from the committee secret key (S...).
    pub fn committee_address(&self) -> Result<String, String> {
        let sk = stellar_strkey::ed25519::PrivateKey::from_string(&self.secret_key)
            .map_err(|e| format!("invalid committee secret key: {:?}", e))?;
        let signing_key = SigningKey::from_bytes(&sk.0);
        let public_key = signing_key.verifying_key().to_bytes();
        Ok(stellar_strkey::ed25519::PublicKey(public_key).to_string())
    }
}

/// Submit a deal proof to the on-chain poker-table contract via `commit_deal`.
pub async fn submit_deal_proof(
    config: &SorobanConfig,
    table_id: u32,
    proof: &[u8],
    public_inputs: &[u8],
    deck_root: &str,
    hand_commitments: &[String],
) -> Result<String, String> {
    if !config.is_configured() {
        tracing::warn!("Soroban not configured, skipping deal proof submission");
        return Ok(String::new());
    }

    let committee_addr = config.committee_address()?;
    let proof_hex = hex::encode(proof);
    let public_inputs_hex = hex::encode(public_inputs);
    let commitments_json = serde_json::to_string(hand_commitments)
        .map_err(|e| format!("Failed to serialize commitments: {}", e))?;

    let output = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &config.poker_table_contract,
            "--source",
            &config.secret_key,
            "--rpc-url",
            &config.rpc_url,
            "--network-passphrase",
            &config.network_passphrase,
            "--",
            "commit_deal",
            "--table_id",
            &table_id.to_string(),
            "--committee",
            &committee_addr,
            "--deck_root",
            deck_root,
            "--hand_commitments",
            &commitments_json,
            "--dealt_indices",
            "[]",
            "--proof",
            &proof_hex,
            "--public_inputs",
            &public_inputs_hex,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to invoke stellar CLI: {}", e))?;

    parse_tx_result(output)
}

/// Submit a reveal proof to the on-chain poker-table contract via `reveal_board`.
pub async fn submit_reveal_proof(
    config: &SorobanConfig,
    table_id: u32,
    proof: &[u8],
    public_inputs: &[u8],
    cards: &[u32],
    indices: &[u32],
) -> Result<String, String> {
    if !config.is_configured() {
        tracing::warn!("Soroban not configured, skipping reveal proof submission");
        return Ok(String::new());
    }

    let committee_addr = config.committee_address()?;
    let proof_hex = hex::encode(proof);
    let public_inputs_hex = hex::encode(public_inputs);
    let cards_json =
        serde_json::to_string(cards).map_err(|e| format!("Failed to serialize cards: {}", e))?;
    let indices_json = serde_json::to_string(indices)
        .map_err(|e| format!("Failed to serialize indices: {}", e))?;

    let output = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &config.poker_table_contract,
            "--source",
            &config.secret_key,
            "--rpc-url",
            &config.rpc_url,
            "--network-passphrase",
            &config.network_passphrase,
            "--",
            "reveal_board",
            "--table_id",
            &table_id.to_string(),
            "--committee",
            &committee_addr,
            "--cards",
            &cards_json,
            "--indices",
            &indices_json,
            "--proof",
            &proof_hex,
            "--public_inputs",
            &public_inputs_hex,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to invoke stellar CLI: {}", e))?;

    parse_tx_result(output)
}

/// Submit a showdown proof to the on-chain poker-table contract via `submit_showdown`.
pub async fn submit_showdown_proof(
    config: &SorobanConfig,
    table_id: u32,
    proof: &[u8],
    public_inputs: &[u8],
    hole_cards: &[(u32, u32)],
) -> Result<String, String> {
    if !config.is_configured() {
        tracing::warn!("Soroban not configured, skipping showdown proof submission");
        return Ok(String::new());
    }

    let committee_addr = config.committee_address()?;
    let proof_hex = hex::encode(proof);
    let public_inputs_hex = hex::encode(public_inputs);
    let hole_cards_json = serde_json::to_string(hole_cards)
        .map_err(|e| format!("Failed to serialize hole cards: {}", e))?;

    let output = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &config.poker_table_contract,
            "--source",
            &config.secret_key,
            "--rpc-url",
            &config.rpc_url,
            "--network-passphrase",
            &config.network_passphrase,
            "--",
            "submit_showdown",
            "--table_id",
            &table_id.to_string(),
            "--committee",
            &committee_addr,
            "--hole_cards",
            &hole_cards_json,
            "--salts",
            "[]",
            "--proof",
            &proof_hex,
            "--public_inputs",
            &public_inputs_hex,
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to invoke stellar CLI: {}", e))?;

    parse_tx_result(output)
}

/// Read on-chain table state via `stellar contract invoke -- get_table`.
pub async fn get_table_state(
    config: &SorobanConfig,
    table_id: u32,
) -> Result<String, String> {
    if !config.is_configured() {
        return Err("Soroban not configured".to_string());
    }

    let output = Command::new("stellar")
        .args([
            "contract",
            "invoke",
            "--id",
            &config.poker_table_contract,
            "--source",
            &config.secret_key,
            "--rpc-url",
            &config.rpc_url,
            "--network-passphrase",
            &config.network_passphrase,
            "--",
            "get_table",
            "--table_id",
            &table_id.to_string(),
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to invoke stellar CLI: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Parse the tx hash from stellar CLI output.
fn parse_tx_result(output: std::process::Output) -> Result<String, String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(format!(
            "stellar contract invoke failed: {}",
            stderr.trim()
        ));
    }

    // The stellar CLI prints the tx hash or result to stdout
    let tx_hash = stdout.trim().to_string();
    if tx_hash.is_empty() {
        // Some versions don't print a hash on success
        Ok("submitted".to_string())
    } else {
        Ok(tx_hash)
    }
}
