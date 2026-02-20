//! Per-table MPC preparation for node-local private contributions.
//!
//! Each node maintains its own secret contribution:
//! - a private permutation of deck indices
//! - a private vector of salt shares
//!
//! The full deck/salts are derived inside Noir from all party contributions.
//! No single node needs plaintext full-deck witness material.

use base64::Engine;
use rand::seq::SliceRandom;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;

const DECK_SIZE: usize = 52;
const MAX_PLAYERS: usize = 6;
const MAX_USED_INDICES: usize = 16;
const MAX_BOARD_INDICES: usize = 5;
const EXPECTED_NOIR_VERSION_PREFIX: &str = "1.0.0-beta.17";

#[derive(Clone, Debug, Default)]
pub struct PrivateTableState {
    contribution: Option<PartyContribution>,
    pending_share_sets: HashMap<String, HashMap<u32, String>>,
}

#[derive(Clone, Debug)]
struct PartyContribution {
    permutation: Vec<u32>,
    salts: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DealPreparation {
    pub share_set_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RevealPreparation {
    pub share_set_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ShowdownPreparation {
    pub share_set_id: String,
}

pub async fn prepare_deal(
    table_id: u32,
    node_id: u32,
    players: &[String],
    circuit_dir: &str,
    tables: &mut HashMap<u32, PrivateTableState>,
) -> Result<DealPreparation, String> {
    if players.len() < 2 || players.len() > MAX_PLAYERS {
        return Err(format!(
            "expected 2..={} players, got {}",
            MAX_PLAYERS,
            players.len()
        ));
    }

    let state = tables.entry(table_id).or_default();
    state.pending_share_sets.clear();
    state.contribution = Some(generate_party_contribution());

    let contribution = state
        .contribution
        .as_ref()
        .ok_or("missing local party contribution")?;

    let input_toml = build_deal_partial_toml(node_id, contribution, players.len() as u32);
    let share_data_by_party = split_partial_input(circuit_dir, "deal_valid", &input_toml).await?;

    let share_set_id = new_share_set_id(table_id);
    state
        .pending_share_sets
        .insert(share_set_id.clone(), share_data_by_party);

    Ok(DealPreparation { share_set_id })
}

pub async fn prepare_reveal(
    table_id: u32,
    node_id: u32,
    phase: &str,
    previously_used_indices: &[u32],
    deck_root: &str,
    circuit_dir: &str,
    tables: &mut HashMap<u32, PrivateTableState>,
) -> Result<RevealPreparation, String> {
    let num_revealed = match phase {
        "flop" => 3u32,
        "turn" => 1u32,
        "river" => 1u32,
        _ => return Err(format!("unsupported reveal phase '{}'", phase)),
    };

    if previously_used_indices.len() > MAX_USED_INDICES {
        return Err(format!(
            "too many previously used indices: {} > {}",
            previously_used_indices.len(),
            MAX_USED_INDICES
        ));
    }

    let state = tables
        .get_mut(&table_id)
        .ok_or_else(|| format!("table {} has no active deal contribution", table_id))?;

    let contribution = state
        .contribution
        .as_ref()
        .ok_or_else(|| format!("table {} has no active deal contribution", table_id))?;

    let input_toml = build_reveal_partial_toml(
        node_id,
        contribution,
        num_revealed,
        previously_used_indices,
        deck_root,
    )?;
    let share_data_by_party =
        split_partial_input(circuit_dir, "reveal_board_valid", &input_toml).await?;

    let share_set_id = new_share_set_id(table_id);
    state
        .pending_share_sets
        .insert(share_set_id.clone(), share_data_by_party);

    Ok(RevealPreparation { share_set_id })
}

pub async fn prepare_showdown(
    table_id: u32,
    node_id: u32,
    board_indices: &[u32],
    num_active_players: u32,
    hand_commitments: &[String],
    deck_root: &str,
    circuit_dir: &str,
    tables: &mut HashMap<u32, PrivateTableState>,
) -> Result<ShowdownPreparation, String> {
    if board_indices.len() != MAX_BOARD_INDICES {
        return Err(format!(
            "showdown requires {} board indices, got {}",
            MAX_BOARD_INDICES,
            board_indices.len()
        ));
    }

    if !(2..=MAX_PLAYERS as u32).contains(&num_active_players) {
        return Err(format!(
            "num_active_players must be 2..={}, got {}",
            MAX_PLAYERS, num_active_players
        ));
    }

    if hand_commitments.len() != num_active_players as usize {
        return Err(format!(
            "hand commitment count {} does not match num_active_players {}",
            hand_commitments.len(),
            num_active_players
        ));
    }

    let state = tables
        .get_mut(&table_id)
        .ok_or_else(|| format!("table {} has no active deal contribution", table_id))?;

    let contribution = state
        .contribution
        .as_ref()
        .ok_or_else(|| format!("table {} has no active deal contribution", table_id))?;

    let input_toml = build_showdown_partial_toml(
        node_id,
        contribution,
        board_indices,
        num_active_players,
        hand_commitments,
        deck_root,
    )?;
    let share_data_by_party = split_partial_input(circuit_dir, "showdown_valid", &input_toml).await?;

    let share_set_id = new_share_set_id(table_id);
    state
        .pending_share_sets
        .insert(share_set_id.clone(), share_data_by_party);

    Ok(ShowdownPreparation { share_set_id })
}

pub fn clone_share_set(
    table_id: u32,
    share_set_id: &str,
    tables: &HashMap<u32, PrivateTableState>,
) -> Result<HashMap<u32, String>, String> {
    let table = tables
        .get(&table_id)
        .ok_or_else(|| format!("unknown table {}", table_id))?;
    table
        .pending_share_sets
        .get(share_set_id)
        .cloned()
        .ok_or_else(|| format!("unknown share_set_id '{}'", share_set_id))
}

pub fn remove_share_set(
    table_id: u32,
    share_set_id: &str,
    tables: &mut HashMap<u32, PrivateTableState>,
) -> Result<(), String> {
    let table = tables
        .get_mut(&table_id)
        .ok_or_else(|| format!("unknown table {}", table_id))?;
    table
        .pending_share_sets
        .remove(share_set_id)
        .ok_or_else(|| format!("unknown share_set_id '{}'", share_set_id))?;
    Ok(())
}

pub async fn dispatch_share_payloads(
    proof_session_id: &str,
    circuit_name: &str,
    peer_http_endpoints: &[String],
    source_party_id: u32,
    share_data_by_party: &HashMap<u32, String>,
) -> Result<(), String> {
    let total_parties = u32::try_from(peer_http_endpoints.len())
        .map_err(|_| "too many peer endpoints".to_string())?;
    let client = reqwest::Client::new();

    let mut handles = Vec::with_capacity(peer_http_endpoints.len());
    for (party_id_usize, endpoint) in peer_http_endpoints.iter().enumerate() {
        let party_id = u32::try_from(party_id_usize)
            .map_err(|_| format!("party index {} out of range", party_id_usize))?;
        let share_data = share_data_by_party
            .get(&party_id)
            .cloned()
            .ok_or_else(|| format!("missing share payload for party {}", party_id))?;

        let url = format!("{}/session/{}/shares", endpoint, proof_session_id);
        let circuit_name = circuit_name.to_string();
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let response = client
                .post(&url)
                .json(&serde_json::json!({
                    "circuit_name": circuit_name,
                    "share_data": share_data,
                    "source_party_id": source_party_id,
                    "total_parties": total_parties,
                }))
                .send()
                .await
                .map_err(|e| format!("dispatch to {} failed: {}", url, e))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "unable to read response body".to_string());
                return Err(format!(
                    "dispatch to {} rejected: HTTP {}: {}",
                    url, status, body
                ));
            }
            Ok::<(), String>(())
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await.map_err(|e| format!("dispatch join error: {}", e))??;
    }
    Ok(())
}

fn generate_party_contribution() -> PartyContribution {
    let mut rng = rand::thread_rng();
    let mut permutation: Vec<u32> = (0..DECK_SIZE as u32).collect();
    permutation.shuffle(&mut rng);

    let salts: Vec<String> = (0..DECK_SIZE)
        .map(|_| format!("{}", rand::random::<u64>()))
        .collect();

    PartyContribution { permutation, salts }
}

fn build_deal_partial_toml(
    node_id: u32,
    contribution: &PartyContribution,
    num_players: u32,
) -> String {
    let mut lines = vec![
        format!(
            "party{}_permutation = {}",
            node_id,
            format_u32_array(&contribution.permutation)
        ),
        format!(
            "party{}_salts = {}",
            node_id,
            format_field_array(&contribution.salts)
        ),
    ];

    if node_id == 0 {
        lines.push(format!("num_players = {}", num_players));
    }

    lines.join("\n") + "\n"
}

fn build_reveal_partial_toml(
    node_id: u32,
    contribution: &PartyContribution,
    num_revealed: u32,
    previously_used_indices: &[u32],
    deck_root: &str,
) -> Result<String, String> {
    let mut padded_used = vec![0u32; MAX_USED_INDICES];
    for (i, idx) in previously_used_indices.iter().enumerate() {
        if i >= MAX_USED_INDICES {
            return Err("too many previously used indices".to_string());
        }
        padded_used[i] = *idx;
    }

    let mut lines = vec![
        format!(
            "party{}_permutation = {}",
            node_id,
            format_u32_array(&contribution.permutation)
        ),
        format!(
            "party{}_salts = {}",
            node_id,
            format_field_array(&contribution.salts)
        ),
    ];

    if node_id == 0 {
        lines.push(format!("deck_root = \"{}\"", deck_root));
        lines.push(format!("num_revealed = {}", num_revealed));
        lines.push(format!(
            "num_previously_used = {}",
            previously_used_indices.len()
        ));
        lines.push(format!(
            "previously_used_indices = {}",
            format_u32_array(&padded_used)
        ));
    }

    Ok(lines.join("\n") + "\n")
}

fn build_showdown_partial_toml(
    node_id: u32,
    contribution: &PartyContribution,
    board_indices: &[u32],
    num_active_players: u32,
    hand_commitments: &[String],
    deck_root: &str,
) -> Result<String, String> {
    if board_indices.len() != MAX_BOARD_INDICES {
        return Err(format!(
            "expected {} board indices, got {}",
            MAX_BOARD_INDICES,
            board_indices.len()
        ));
    }

    let mut padded_commitments = vec!["0".to_string(); MAX_PLAYERS];
    for (i, c) in hand_commitments.iter().enumerate() {
        if i >= MAX_PLAYERS {
            return Err(format!("too many hand commitments: {}", hand_commitments.len()));
        }
        padded_commitments[i] = c.clone();
    }

    let mut lines = vec![
        format!(
            "party{}_permutation = {}",
            node_id,
            format_u32_array(&contribution.permutation)
        ),
        format!(
            "party{}_salts = {}",
            node_id,
            format_field_array(&contribution.salts)
        ),
    ];

    if node_id == 0 {
        lines.push(format!("num_active_players = {}", num_active_players));
        lines.push(format!(
            "hand_commitments = {}",
            format_field_array(&padded_commitments)
        ));
        lines.push(format!("board_indices = {}", format_u32_array(board_indices)));
        lines.push(format!("deck_root = \"{}\"", deck_root));
    }

    Ok(lines.join("\n") + "\n")
}

async fn split_partial_input(
    circuit_dir: &str,
    circuit_name: &str,
    input_toml: &str,
) -> Result<HashMap<u32, String>, String> {
    let tmp = tempfile::tempdir().map_err(|e| format!("tmpdir: {}", e))?;
    let input_path = tmp.path().join("partial.toml");
    let out_dir = tmp.path().join("split");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("create out dir: {}", e))?;
    std::fs::write(&input_path, input_toml).map_err(|e| format!("write partial input: {}", e))?;

    let circuit_path = format!("{}/{}/target/{}.json", circuit_dir, circuit_name, circuit_name);
    validate_circuit_artifact_compatibility(&circuit_path)?;

    let output = Command::new("co-noir")
        .arg("split-input")
        .arg("--circuit")
        .arg(&circuit_path)
        .arg("--input")
        .arg(&input_path)
        .arg("--protocol")
        .arg("REP3")
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .await
        .map_err(|e| format!("failed to spawn co-noir split-input: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "co-noir split-input failed:\nstderr: {}\nstdout: {}",
            stderr, stdout
        ));
    }

    collect_split_shares(&out_dir)
}

fn collect_split_shares(out_dir: &Path) -> Result<HashMap<u32, String>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(out_dir)
        .map_err(|e| format!("read split output dir: {}", e))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|s| s.to_str())
                .map(|name| name.ends_with(".shared"))
                .unwrap_or(false)
        })
        .collect();

    if files.is_empty() {
        return Err("co-noir split-input produced no .shared files".to_string());
    }

    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    let mut share_data_by_party: HashMap<u32, String> = HashMap::new();

    for (fallback_idx, path) in files.iter().enumerate() {
        let bytes =
            std::fs::read(path).map_err(|e| format!("failed to read share file {:?}: {}", path, e))?;
        let share_b64 = base64::engine::general_purpose::STANDARD.encode(bytes);

        let party_id = parse_party_id_from_share_filename(path)
            .unwrap_or_else(|| u32::try_from(fallback_idx).unwrap_or(0));

        if share_data_by_party.insert(party_id, share_b64).is_some() {
            return Err(format!(
                "duplicate party id {} in split output {:?}",
                party_id, path
            ));
        }
    }

    Ok(share_data_by_party)
}

fn parse_party_id_from_share_filename(path: &Path) -> Option<u32> {
    let file_name = path.file_name()?.to_str()?;
    let without_suffix = file_name.strip_suffix(".shared")?;
    let (_, idx) = without_suffix.rsplit_once('.')?;
    idx.parse::<u32>().ok()
}

fn validate_circuit_artifact_compatibility(circuit_path: &str) -> Result<(), String> {
    let artifact_raw = std::fs::read_to_string(circuit_path)
        .map_err(|e| format!("failed to read circuit artifact '{}': {}", circuit_path, e))?;
    let artifact_json: serde_json::Value = serde_json::from_str(&artifact_raw).map_err(|e| {
        format!(
            "failed to parse circuit artifact '{}' as json: {}",
            circuit_path, e
        )
    })?;

    let noir_version = artifact_json
        .get("noir_version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            format!(
                "circuit artifact '{}' is missing noir_version metadata",
                circuit_path
            )
        })?;

    if !noir_version.starts_with(EXPECTED_NOIR_VERSION_PREFIX) {
        return Err(format!(
            "circuit artifact '{}' noir_version='{}' is incompatible with co-noir parser expectations (need '{}*'). Recompile with ./scripts/compile-circuits.sh",
            circuit_path,
            noir_version,
            EXPECTED_NOIR_VERSION_PREFIX
        ));
    }

    Ok(())
}

fn format_u32_array(values: &[u32]) -> String {
    let joined = values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{}]", joined)
}

fn format_field_array(values: &[String]) -> String {
    let joined = values
        .iter()
        .map(|v| format!("\"{}\"", v))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{}]", joined)
}

fn new_share_set_id(table_id: u32) -> String {
    format!("table-{}-shares-{}", table_id, rand::random::<u64>())
}
