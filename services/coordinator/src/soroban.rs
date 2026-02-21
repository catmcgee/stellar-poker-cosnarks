//! Soroban on-chain proof submission via `stellar contract invoke`.
//!
//! Shells out to the Stellar CLI to submit proofs and game state to
//! the on-chain poker-table contract. Uses the same `tokio::process::Command`
//! pattern as `mpc.rs` for co-noir subprocess execution.

use std::str::FromStr;

use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};
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
    pub onchain_table_id: Option<u32>,
    pub player_identities: Vec<(String, String)>,
}

impl SorobanConfig {
    pub fn from_env() -> Self {
        let mut player_identities = Vec::new();
        for idx in 1..=6 {
            let address_key = format!("PLAYER{}_ADDRESS", idx);
            if let Ok(address) = std::env::var(&address_key) {
                if address.trim().is_empty() {
                    continue;
                }
                let identity_key = format!("PLAYER{}_IDENTITY", idx);
                let identity = std::env::var(&identity_key)
                    .unwrap_or_else(|_| format!("player{}-local", idx));
                player_identities.push((address, identity));
            }
        }

        Self {
            rpc_url: std::env::var("SOROBAN_RPC")
                .unwrap_or_else(|_| "http://localhost:8000/soroban/rpc".to_string()),
            secret_key: std::env::var("COMMITTEE_SECRET")
                .unwrap_or_else(|_| "test_secret".to_string()),
            poker_table_contract: std::env::var("POKER_TABLE_CONTRACT")
                .unwrap_or_else(|_| String::new()),
            network_passphrase: std::env::var("NETWORK_PASSPHRASE")
                .unwrap_or_else(|_| "Test SDF Network ; September 2015".to_string()),
            onchain_table_id: std::env::var("ONCHAIN_TABLE_ID")
                .ok()
                .or_else(|| std::env::var("TABLE_ID").ok())
                .and_then(|s| s.parse().ok()),
            player_identities,
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

    fn identity_for_player(&self, player_address: &str) -> Option<&str> {
        self.player_identities
            .iter()
            .find(|(address, _)| address == player_address)
            .map(|(_, identity)| identity.as_str())
    }
}

const INSTRUCTION_LEEWAY_STEPS: [u64; 4] = [0, 50_000_000, 200_000_000, 500_000_000];

fn is_transient_invoke_error(output: &std::process::Output) -> bool {
    if output.status.success() {
        return false;
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    stderr.contains("resourcelimitexceeded")
        || stderr.contains("connection reset by peer")
        || stderr.contains("timed out")
        || stderr.contains("timeout")
        || stderr.contains("temporarily unavailable")
        || stderr.contains("networking or low-level protocol error")
}

async fn invoke_contract_with_retries(
    config: &SorobanConfig,
    contract_args: Vec<String>,
) -> Result<std::process::Output, String> {
    let mut last_output: Option<std::process::Output> = None;

    for (attempt_idx, leeway) in INSTRUCTION_LEEWAY_STEPS.iter().enumerate() {
        let mut args: Vec<String> = vec![
            "contract".to_string(),
            "invoke".to_string(),
            "--id".to_string(),
            config.poker_table_contract.clone(),
            "--source".to_string(),
            config.secret_key.clone(),
            "--rpc-url".to_string(),
            config.rpc_url.clone(),
            "--network-passphrase".to_string(),
            config.network_passphrase.clone(),
        ];

        if *leeway > 0 {
            args.push("--instruction-leeway".to_string());
            args.push(leeway.to_string());
        }

        args.push("--".to_string());
        args.extend(contract_args.iter().cloned());

        let output = Command::new("stellar")
            .args(&args)
            .output()
            .await
            .map_err(|e| format!("Failed to invoke stellar CLI: {}", e))?;

        if output.status.success() {
            return Ok(output);
        }

        let is_resource_limit = is_transient_invoke_error(&output)
            && String::from_utf8_lossy(&output.stderr).contains("ResourceLimitExceeded");
        let has_next_attempt = attempt_idx + 1 < INSTRUCTION_LEEWAY_STEPS.len();

        if is_resource_limit && has_next_attempt {
            tracing::warn!(
                "stellar invoke hit ResourceLimitExceeded; retrying with higher instruction leeway (attempt {}/{})",
                attempt_idx + 1,
                INSTRUCTION_LEEWAY_STEPS.len()
            );
            last_output = Some(output);
            continue;
        }

        return Ok(output);
    }

    last_output.ok_or_else(|| "stellar invoke failed before any attempt completed".to_string())
}

fn resolve_onchain_table_id(config: &SorobanConfig, table_id: u32) -> u32 {
    if table_id == 0 {
        config.onchain_table_id.unwrap_or(0)
    } else {
        table_id
    }
}

async fn invoke_contract_with_source(
    config: &SorobanConfig,
    source: &str,
    contract_args: Vec<String>,
) -> Result<std::process::Output, String> {
    let mut args: Vec<String> = vec![
        "contract".to_string(),
        "invoke".to_string(),
        "--id".to_string(),
        config.poker_table_contract.clone(),
        "--source".to_string(),
        source.to_string(),
        "--rpc-url".to_string(),
        config.rpc_url.clone(),
        "--network-passphrase".to_string(),
        config.network_passphrase.clone(),
        "--".to_string(),
    ];
    args.extend(contract_args);

    Command::new("stellar")
        .args(&args)
        .output()
        .await
        .map_err(|e| format!("Failed to invoke stellar CLI: {}", e))
}

async fn invoke_contract_with_source_retries(
    config: &SorobanConfig,
    source: &str,
    contract_args: Vec<String>,
) -> Result<std::process::Output, String> {
    const MAX_RETRIES: usize = 3;
    let mut last_output: Option<std::process::Output> = None;

    for attempt in 1..=MAX_RETRIES {
        let output = invoke_contract_with_source(config, source, contract_args.clone()).await?;
        if output.status.success() {
            return Ok(output);
        }

        let should_retry = is_transient_invoke_error(&output) && attempt < MAX_RETRIES;
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        tracing::warn!(
            "stellar invoke (source={}, attempt {}/{}) failed{}: {}",
            source,
            attempt,
            MAX_RETRIES,
            if should_retry { ", retrying" } else { "" },
            stderr.trim()
        );

        if !should_retry {
            return Ok(output);
        }

        last_output = Some(output);
        tokio::time::sleep(std::time::Duration::from_millis(300 * attempt as u64)).await;
    }

    last_output.ok_or_else(|| "stellar invoke failed before any attempt completed".to_string())
}

fn parse_i128_value(value: &serde_json::Value) -> Option<i128> {
    match value {
        serde_json::Value::String(s) => s.parse::<i128>().ok(),
        serde_json::Value::Number(n) => n.as_i64().map(|v| v as i128),
        _ => None,
    }
}

/// When reveal is requested directly from the frontend, advance one legal betting
/// action if the on-chain table is still in a betting phase.
pub async fn maybe_auto_advance_betting_for_reveal(
    config: &SorobanConfig,
    table_id: u32,
    reveal_phase: &str,
) -> Result<(), String> {
    if !config.is_configured() {
        return Ok(());
    }

    let expected = match reveal_phase {
        "flop" => "Preflop",
        "turn" => "Flop",
        "river" => "Turn",
        _ => return Ok(()),
    };

    maybe_auto_advance_betting_if_phase(config, table_id, expected, "reveal").await
}

/// When showdown is requested directly from the frontend, advance one legal
/// betting action if the on-chain table is still in River betting.
pub async fn maybe_auto_advance_betting_for_showdown(
    config: &SorobanConfig,
    table_id: u32,
) -> Result<(), String> {
    if !config.is_configured() {
        return Ok(());
    }
    maybe_auto_advance_betting_if_phase(config, table_id, "River", "showdown").await
}

async fn maybe_auto_advance_betting_if_phase(
    config: &SorobanConfig,
    table_id: u32,
    expected_phase: &str,
    reason: &str,
) -> Result<(), String> {
    const MAX_AUTO_ACTIONS: usize = 24;

    for step in 0..MAX_AUTO_ACTIONS {
        let state_raw = get_table_state(config, table_id).await?;
        let state: serde_json::Value = serde_json::from_str(&state_raw)
            .map_err(|e| format!("failed to parse on-chain table state: {}", e))?;

        let phase = state
            .get("phase")
            .and_then(|v| v.as_str())
            .ok_or("missing phase in on-chain table state")?;

        if phase != expected_phase {
            return Ok(());
        }

        let players = state
            .get("players")
            .and_then(|v| v.as_array())
            .ok_or("missing players in on-chain table state")?;
        let current_turn = state
            .get("current_turn")
            .and_then(|v| v.as_u64())
            .ok_or("missing current_turn in on-chain table state")? as usize;

        let current_player = players
            .get(current_turn)
            .ok_or("current_turn out of range for on-chain players")?;
        let player_address = current_player
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or("missing current player address")?;
        let source_identity = config.identity_for_player(player_address).ok_or_else(|| {
            format!(
                "no local identity configured for player {} (set PLAYERn_ADDRESS/PLAYERn_IDENTITY)",
                player_address
            )
        })?;

        let current_bet = players
            .iter()
            .filter_map(|p| p.get("bet_this_round"))
            .filter_map(parse_i128_value)
            .max()
            .unwrap_or(0);
        let my_bet = current_player
            .get("bet_this_round")
            .and_then(parse_i128_value)
            .unwrap_or(0);

        let action_json = if my_bet < current_bet { "\"Call\"" } else { "\"Check\"" };
        let onchain_table_id = resolve_onchain_table_id(config, table_id);
        tracing::info!(
            "Auto-advancing betting before {}: phase={}, action={}, player={}, step={}",
            reason,
            phase,
            action_json,
            player_address,
            step + 1
        );

        let output = invoke_contract_with_source_retries(
            config,
            source_identity,
            vec![
                "player_action".to_string(),
                "--table_id".to_string(),
                onchain_table_id.to_string(),
                "--player".to_string(),
                player_address.to_string(),
                "--action".to_string(),
                action_json.to_string(),
            ],
        )
        .await?;

        parse_tx_result(output)?;
    }

    Err(format!(
        "auto-advance before {} exceeded {} actions while phase remained {}",
        reason, MAX_AUTO_ACTIONS, expected_phase
    ))
}

/// Submit a deal proof to the on-chain poker-table contract via `commit_deal`.
pub async fn submit_deal_proof(
    config: &SorobanConfig,
    table_id: u32,
    proof: &[u8],
    public_inputs: &[String],
    deck_root: &str,
    hand_commitments: &[String],
) -> Result<String, String> {
    if !config.is_configured() {
        tracing::warn!("Soroban not configured, skipping deal proof submission");
        return Ok(String::new());
    }

    maybe_start_hand_for_deal(config, table_id).await?;

    let onchain_table_id = resolve_onchain_table_id(config, table_id);
    let committee_addr = config.committee_address()?;
    let converted_proof = convert_keccak_proof_to_soroban(proof)?;
    let proof_hex = hex::encode(&converted_proof);
    let pi_hex = public_inputs_to_hex(public_inputs)?;
    let deck_root_hex = field_to_bytes32_hex(deck_root)?;
    let commitments_hex_json = fields_to_bytes32_json(hand_commitments)?;

    tracing::info!(
        "Soroban deal proof: raw_bytes={}, converted_bytes={}, public_inputs_count={}, pi_hex_bytes={}, deck_root_hex={}, commitments_json={}",
        proof.len(),
        converted_proof.len(),
        public_inputs.len(),
        pi_hex.len() / 2,
        deck_root_hex,
        commitments_hex_json,
    );

    let output = invoke_contract_with_retries(
        config,
        vec![
            "commit_deal".to_string(),
            "--table_id".to_string(),
            onchain_table_id.to_string(),
            "--committee".to_string(),
            committee_addr,
            "--deck_root".to_string(),
            deck_root_hex,
            "--hand_commitments".to_string(),
            commitments_hex_json,
            "--dealt_indices".to_string(),
            "[]".to_string(),
            "--proof".to_string(),
            proof_hex,
            "--public_inputs".to_string(),
            pi_hex,
        ],
    )
    .await?;

    parse_tx_result(output)
}

async fn maybe_start_hand_for_deal(config: &SorobanConfig, table_id: u32) -> Result<(), String> {
    let state_raw = get_table_state(config, table_id).await?;
    let state: serde_json::Value = serde_json::from_str(&state_raw)
        .map_err(|e| format!("failed to parse on-chain table state: {}", e))?;

    let phase = state
        .get("phase")
        .and_then(|v| v.as_str())
        .ok_or("missing phase in on-chain table state")?;

    match phase {
        "Dealing" => return Ok(()),
        "Waiting" | "Settlement" => {}
        _ => {
            return Err(format!(
                "table {} not ready for new deal; current phase is {}",
                table_id, phase
            ))
        }
    }

    let onchain_table_id = resolve_onchain_table_id(config, table_id);
    tracing::info!(
        "Auto-starting hand before deal submission: table_id={}, phase={}",
        onchain_table_id,
        phase
    );
    let output = invoke_contract_with_retries(
        config,
        vec![
            "start_hand".to_string(),
            "--table_id".to_string(),
            onchain_table_id.to_string(),
        ],
    )
    .await?;
    parse_tx_result(output).map(|_| ())
}

/// Submit a reveal proof to the on-chain poker-table contract via `reveal_board`.
pub async fn submit_reveal_proof(
    config: &SorobanConfig,
    table_id: u32,
    proof: &[u8],
    public_inputs: &[String],
    cards: &[u32],
    indices: &[u32],
) -> Result<String, String> {
    if !config.is_configured() {
        tracing::warn!("Soroban not configured, skipping reveal proof submission");
        return Ok(String::new());
    }

    let onchain_table_id = resolve_onchain_table_id(config, table_id);
    let committee_addr = config.committee_address()?;
    let converted_proof = convert_keccak_proof_to_soroban(proof)?;
    let proof_hex = hex::encode(&converted_proof);
    let pi_hex = public_inputs_to_hex(public_inputs)?;
    let cards_json =
        serde_json::to_string(cards).map_err(|e| format!("Failed to serialize cards: {}", e))?;
    let indices_json = serde_json::to_string(indices)
        .map_err(|e| format!("Failed to serialize indices: {}", e))?;

    let output = invoke_contract_with_retries(
        config,
        vec![
            "reveal_board".to_string(),
            "--table_id".to_string(),
            onchain_table_id.to_string(),
            "--committee".to_string(),
            committee_addr,
            "--cards".to_string(),
            cards_json,
            "--indices".to_string(),
            indices_json,
            "--proof".to_string(),
            proof_hex,
            "--public_inputs".to_string(),
            pi_hex,
        ],
    )
    .await?;

    parse_tx_result(output)
}

/// Submit a showdown proof to the on-chain poker-table contract via `submit_showdown`.
pub async fn submit_showdown_proof(
    config: &SorobanConfig,
    table_id: u32,
    proof: &[u8],
    public_inputs: &[String],
    hole_cards: &[(u32, u32)],
) -> Result<String, String> {
    if !config.is_configured() {
        tracing::warn!("Soroban not configured, skipping showdown proof submission");
        return Ok(String::new());
    }

    let onchain_table_id = resolve_onchain_table_id(config, table_id);
    let committee_addr = config.committee_address()?;
    let converted_proof = convert_keccak_proof_to_soroban(proof)?;
    let proof_hex = hex::encode(&converted_proof);
    let pi_hex = public_inputs_to_hex(public_inputs)?;
    let hole_cards_json = serde_json::to_string(hole_cards)
        .map_err(|e| format!("Failed to serialize hole cards: {}", e))?;

    let output = invoke_contract_with_retries(
        config,
        vec![
            "submit_showdown".to_string(),
            "--table_id".to_string(),
            onchain_table_id.to_string(),
            "--committee".to_string(),
            committee_addr,
            "--hole_cards".to_string(),
            hole_cards_json,
            "--salts".to_string(),
            "[]".to_string(),
            "--proof".to_string(),
            proof_hex,
            "--public_inputs".to_string(),
            pi_hex,
        ],
    )
    .await?;

    parse_tx_result(output)
}

/// Submit a timeout claim to force committee-failure settlement when a hand is stuck.
pub async fn claim_timeout(config: &SorobanConfig, table_id: u32) -> Result<String, String> {
    if !config.is_configured() {
        return Err("Soroban not configured".to_string());
    }

    let onchain_table_id = resolve_onchain_table_id(config, table_id);
    let claimer = config.committee_address()?;
    let output = invoke_contract_with_retries(
        config,
        vec![
            "claim_timeout".to_string(),
            "--table_id".to_string(),
            onchain_table_id.to_string(),
            "--claimer".to_string(),
            claimer,
        ],
    )
    .await?;

    parse_tx_result(output)
}

/// Create a new table by cloning the reference table config and pre-seeding
/// seats with configured local player identities.
pub async fn create_seeded_table(
    config: &SorobanConfig,
    reference_table_id: u32,
    max_players: u32,
    buy_in: i128,
) -> Result<u32, String> {
    if !config.is_configured() {
        return Err("Soroban not configured".to_string());
    }
    if !(2..=6).contains(&max_players) {
        return Err(format!("max_players out of range: {}", max_players));
    }
    if config.player_identities.len() < max_players as usize {
        return Err(format!(
            "not enough configured player identities: have {}, need {}",
            config.player_identities.len(),
            max_players
        ));
    }

    let raw = get_table_state(config, reference_table_id).await?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("failed to parse reference table: {}", e))?;
    let mut cfg = value
        .get("config")
        .cloned()
        .ok_or("reference table missing config")?;
    if let Some(obj) = cfg.as_object_mut() {
        obj.insert(
            "max_players".to_string(),
            serde_json::Value::Number(serde_json::Number::from(max_players)),
        );
        obj.insert(
            "committee".to_string(),
            serde_json::Value::String(config.committee_address()?),
        );
    } else {
        return Err("reference config is not an object".to_string());
    }
    let cfg_json = serde_json::to_string(&cfg)
        .map_err(|e| format!("failed to serialize table config: {}", e))?;

    let committee_addr = config.committee_address()?;
    let output = invoke_contract_with_retries(
        config,
        vec![
            "create_table".to_string(),
            "--admin".to_string(),
            committee_addr,
            "--config".to_string(),
            cfg_json,
        ],
    )
    .await?;

    if !output.status.success() {
        return Err(format!(
            "create_table failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let table_id = parse_u32_from_stdout(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| "failed to parse table id from create_table output".to_string())?;

    let onchain_table_id = resolve_onchain_table_id(config, table_id);
    for (player_address, identity) in config.player_identities.iter().take(max_players as usize) {
        let join_output = invoke_contract_with_source_retries(
            config,
            identity,
            vec![
                "join_table".to_string(),
                "--table_id".to_string(),
                onchain_table_id.to_string(),
                "--player".to_string(),
                player_address.clone(),
                "--buy_in".to_string(),
                buy_in.to_string(),
            ],
        )
        .await?;

        parse_tx_result(join_output)?;
    }

    Ok(table_id)
}

/// Read on-chain table state via `stellar contract invoke -- get_table`.
pub async fn get_table_state(
    config: &SorobanConfig,
    table_id: u32,
) -> Result<String, String> {
    if !config.is_configured() {
        return Err("Soroban not configured".to_string());
    }

    let onchain_table_id = resolve_onchain_table_id(config, table_id);
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
            &onchain_table_id.to_string(),
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

/// Convert co-noir keccak proof format to the Soroban/BB UltraHonk verifier format.
///
/// co-noir keccak format (variable size, raw G1 coordinates):
///   [pairing_points(16 Fr), G1_raw(8×2), sumcheck_uni(log_n×8),
///    sumcheck_eval(41), gemini_fold_raw((log_n-1)×2), gemini_eval(log_n),
///    shplonk_raw(1×2), kzg_raw(1×2)]
///
/// Soroban verifier format (fixed 458 fields, limb-encoded G1):
///   [pairing_points(16), G1_limb(8×4), sumcheck_uni(28×8),
///    sumcheck_eval(41), gemini_fold_limb(27×4), gemini_eval(28),
///    shplonk_limb(1×4), kzg_limb(1×4), log_n(1)]
fn convert_keccak_proof_to_soroban(proof_bytes: &[u8]) -> Result<Vec<u8>, String> {
    const FIELD_SIZE: usize = 32;
    const SOROBAN_PROOF_FIELDS: usize = 458;
    const SOROBAN_PROOF_BYTES: usize = SOROBAN_PROOF_FIELDS * FIELD_SIZE;
    const CONST_PROOF_SIZE_LOG_N: usize = 28;
    const BATCHED_RELATION_PARTIAL_LENGTH: usize = 8;
    const NUMBER_OF_ENTITIES: usize = 41;
    const NUM_G1_WIRE_POINTS: usize = 8;
    const NUM_FINAL_G1: usize = 2;
    const PAIRING_POINTS_SIZE: usize = 16;

    if proof_bytes.len() % FIELD_SIZE != 0 {
        return Err(format!("proof not 32-byte aligned: {} bytes", proof_bytes.len()));
    }

    let num_fields = proof_bytes.len() / FIELD_SIZE;

    // Derive log_n from proof size:
    // total = PAIRING + G1_RAW + SUMCHECK + EVALS + GEMINI_FOLD + GEMINI_EVAL + FINAL_G1
    // total = 16 + 16 + log_n*8 + 41 + (log_n-1)*2 + log_n + 4
    // total = 77 + log_n*8 + (log_n-1)*2 + log_n
    // total = 77 + 11*log_n - 2
    // total = 75 + 11*log_n
    // log_n = (total - 75) / 11
    let log_n_calc = num_fields as i64 - 75;
    if log_n_calc <= 0 || log_n_calc % 11 != 0 {
        return Err(format!(
            "cannot derive log_n from proof size: {} fields (remainder {})",
            num_fields, log_n_calc % 11
        ));
    }
    let log_n = (log_n_calc / 11) as usize;

    // Verify derived log_n is reasonable
    if log_n < 10 || log_n > 25 {
        return Err(format!("derived log_n={} out of reasonable range [10,25]", log_n));
    }

    // Verify total
    let expected = PAIRING_POINTS_SIZE + NUM_G1_WIRE_POINTS * 2
        + log_n * BATCHED_RELATION_PARTIAL_LENGTH + NUMBER_OF_ENTITIES
        + (log_n - 1) * 2 + log_n + NUM_FINAL_G1 * 2;
    if num_fields != expected {
        return Err(format!(
            "proof size mismatch: got {} fields, expected {} (log_n={})",
            num_fields, expected, log_n
        ));
    }

    tracing::info!("Proof conversion: {} fields, derived log_n={}", num_fields, log_n);

    let mut out = Vec::with_capacity(SOROBAN_PROOF_BYTES);
    let mut offset = 0usize;

    // Helper: read 32 bytes from proof
    let read_fr = |off: &mut usize| -> &[u8] {
        let start = *off;
        *off += FIELD_SIZE;
        &proof_bytes[start..start + FIELD_SIZE]
    };

    // Helper: split a 32-byte big-endian coordinate into (lo136, hi) limb pair
    fn coord_to_limbs(coord: &[u8]) -> ([u8; 32], [u8; 32]) {
        let mut lo = [0u8; 32];
        let mut hi = [0u8; 32];
        lo[15..].copy_from_slice(&coord[15..]); // lower 17 bytes
        hi[17..].copy_from_slice(&coord[..15]); // upper 15 bytes
        (lo, hi)
    }

    // Helper: convert raw G1 (x, y) to limb-encoded (x_lo, x_hi, y_lo, y_hi)
    let convert_g1_raw_to_limb = |off: &mut usize, out: &mut Vec<u8>| {
        let x = &proof_bytes[*off..*off + FIELD_SIZE];
        *off += FIELD_SIZE;
        let y = &proof_bytes[*off..*off + FIELD_SIZE];
        *off += FIELD_SIZE;
        let (x_lo, x_hi) = coord_to_limbs(x);
        let (y_lo, y_hi) = coord_to_limbs(y);
        out.extend_from_slice(&x_lo);
        out.extend_from_slice(&x_hi);
        out.extend_from_slice(&y_lo);
        out.extend_from_slice(&y_hi);
    };

    // 1) Pairing point object: 16 Fr values — these are limb-encoded accumulator
    //    coordinates in both formats, copy directly
    for _ in 0..PAIRING_POINTS_SIZE {
        out.extend_from_slice(read_fr(&mut offset));
    }

    // 2) 8 G1 wire commitments: convert from raw (x,y) to limb (x_lo,x_hi,y_lo,y_hi)
    for _ in 0..NUM_G1_WIRE_POINTS {
        convert_g1_raw_to_limb(&mut offset, &mut out);
    }

    // 3) Sumcheck univariates: log_n rounds → pad to CONST_PROOF_SIZE_LOG_N
    for _ in 0..log_n {
        for _ in 0..BATCHED_RELATION_PARTIAL_LENGTH {
            out.extend_from_slice(read_fr(&mut offset));
        }
    }
    let pad_rounds = CONST_PROOF_SIZE_LOG_N - log_n;
    out.extend(vec![0u8; pad_rounds * BATCHED_RELATION_PARTIAL_LENGTH * FIELD_SIZE]);

    // 4) Sumcheck evaluations: 41 Fr (copy directly)
    for _ in 0..NUMBER_OF_ENTITIES {
        out.extend_from_slice(read_fr(&mut offset));
    }

    // 5) Gemini fold comms: (log_n-1) raw G1 → limb-encode, pad to 27
    for _ in 0..(log_n - 1) {
        convert_g1_raw_to_limb(&mut offset, &mut out);
    }
    let pad_gemini = (CONST_PROOF_SIZE_LOG_N - 1) - (log_n - 1);
    out.extend(vec![0u8; pad_gemini * 4 * FIELD_SIZE]);

    // 6) Gemini a evaluations: log_n Fr → pad to CONST_PROOF_SIZE_LOG_N
    for _ in 0..log_n {
        out.extend_from_slice(read_fr(&mut offset));
    }
    out.extend(vec![0u8; (CONST_PROOF_SIZE_LOG_N - log_n) * FIELD_SIZE]);

    // 7) Shplonk Q and KZG quotient: 2 raw G1 → limb-encode
    for _ in 0..NUM_FINAL_G1 {
        convert_g1_raw_to_limb(&mut offset, &mut out);
    }

    // 8) Append log_n as final field (big-endian u256)
    let mut log_n_field = [0u8; 32];
    log_n_field[31] = log_n as u8;
    if log_n > 255 {
        log_n_field[30] = (log_n >> 8) as u8;
    }
    out.extend_from_slice(&log_n_field);

    // Verify we consumed all input (except preamble already skipped)
    if offset != proof_bytes.len() {
        return Err(format!(
            "proof conversion: consumed {} of {} bytes ({} fields leftover)",
            offset, proof_bytes.len(), (proof_bytes.len() - offset) / FIELD_SIZE
        ));
    }

    if out.len() != SOROBAN_PROOF_BYTES {
        return Err(format!(
            "converted proof size mismatch: got {} bytes, expected {}",
            out.len(), SOROBAN_PROOF_BYTES
        ));
    }

    tracing::info!(
        "Proof converted: {} bytes (keccak, log_n={}) → {} bytes (soroban)",
        proof_bytes.len(), log_n,
        out.len()
    );

    Ok(out)
}

/// Convert a BN254 field element (decimal string) to a 32-byte big-endian hex string.
/// This is needed because Soroban `BytesN<32>` expects hex-encoded bytes, but
/// MPC proof outputs are decimal field element strings.
fn field_to_bytes32_hex(field_str: &str) -> Result<String, String> {
    let fr = Fr::from_str(field_str)
        .map_err(|_| format!("failed to parse field element: '{}'", field_str))?;
    let bytes = fr.into_bigint().to_bytes_be();
    // Pad to exactly 32 bytes (should already be, but be safe)
    if bytes.len() > 32 {
        return Err(format!("field element too large: {} bytes", bytes.len()));
    }
    let mut padded = vec![0u8; 32 - bytes.len()];
    padded.extend_from_slice(&bytes);
    Ok(hex::encode(padded))
}

/// Convert a slice of field element strings to a JSON array of hex-encoded BytesN<32>.
fn fields_to_bytes32_json(fields: &[String]) -> Result<String, String> {
    let hex_strings: Vec<String> = fields
        .iter()
        .map(|f| field_to_bytes32_hex(f))
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::to_string(&hex_strings)
        .map_err(|e| format!("failed to serialize hex array: {}", e))
}

/// Convert proof public inputs (field element strings) to concatenated 32-byte big-endian
/// representations suitable for the on-chain verifier.
fn public_inputs_to_hex(public_inputs: &[String]) -> Result<String, String> {
    let mut all_bytes = Vec::with_capacity(public_inputs.len() * 32);
    for pi in public_inputs {
        let fr = Fr::from_str(pi)
            .map_err(|_| format!("failed to parse public input: '{}'", pi))?;
        let bytes = fr.into_bigint().to_bytes_be();
        let mut padded = vec![0u8; 32 - bytes.len()];
        padded.extend_from_slice(&bytes);
        all_bytes.extend_from_slice(&padded);
    }
    Ok(hex::encode(all_bytes))
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

fn parse_u32_from_stdout(stdout: &str) -> Option<u32> {
    for line in stdout.lines().rev() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if let Ok(v) = t.parse::<u32>() {
            return Some(v);
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(t) {
            if let Some(v) = json.as_u64().and_then(|n| u32::try_from(n).ok()) {
                return Some(v);
            }
            if let Some(v) = json
                .get("u32")
                .and_then(|n| n.as_u64())
                .and_then(|n| u32::try_from(n).ok())
            {
                return Some(v);
            }
        }
    }
    None
}
