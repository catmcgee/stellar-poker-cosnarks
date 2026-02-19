//! MPC session manager for co-noir proof generation.
//!
//! Each session represents one proof generation request (deal, reveal, or showdown).
//! The lifecycle:
//! 1. Coordinator sends shares via POST /session/:id/shares
//! 2. Coordinator triggers proof gen via POST /session/:id/generate
//! 3. Node runs co-noir witness extension + proof generation as subprocesses
//! 4. Coordinator polls GET /session/:id/status and retrieves proof

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    /// Shares received, waiting for generate trigger
    SharesReceived,
    /// Witness extension in progress
    WitnessGenerating,
    /// Proof generation in progress
    ProofGenerating,
    /// Proof generation complete
    Complete,
    /// Something failed
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct MpcSessionState {
    pub session_id: String,
    pub circuit_name: String,
    pub status: SessionStatus,
    /// Path to the received share file (Prover.toml with secret-shared values)
    pub share_path: Option<PathBuf>,
    /// Working directory for this session's temp files
    pub work_dir: PathBuf,
    /// Path to generated witness
    pub witness_path: Option<PathBuf>,
    /// Path to generated proof
    pub proof_path: Option<PathBuf>,
}

impl MpcSessionState {
    pub fn new(session_id: String, circuit_name: String, work_dir: PathBuf) -> Self {
        Self {
            session_id,
            circuit_name,
            status: SessionStatus::SharesReceived,
            share_path: None,
            work_dir,
            witness_path: None,
            proof_path: None,
        }
    }
}

/// Save base64-decoded share data to a file in the session's work directory.
pub fn receive_shares(session: &mut MpcSessionState, share_data_b64: &str) -> Result<(), String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(share_data_b64)
        .map_err(|e| format!("base64 decode error: {}", e))?;

    let share_path = session.work_dir.join("Prover.toml");
    std::fs::write(&share_path, &bytes)
        .map_err(|e| format!("failed to write share file: {}", e))?;

    session.share_path = Some(share_path);
    session.status = SessionStatus::SharesReceived;
    Ok(())
}

/// Run co-noir proof generation as async subprocesses.
///
/// This spawns two sequential commands:
/// 1. `co-noir generate-witness` — extends the witness in MPC
/// 2. `co-noir build-and-generate-proof` — generates the UltraHonk proof in MPC
///
/// co-noir handles all peer-to-peer MPC communication internally via TCP.
pub async fn run_proof_generation(
    session_id: String,
    circuit_dir: String,
    circuit_name: String,
    work_dir: PathBuf,
    node_id: u32,
    party_config_path: String,
    crs_path: String,
) -> Result<Vec<u8>, String> {
    let circuit_path = format!("{}/{}/target/{}.json", circuit_dir, circuit_name, circuit_name);
    let share_path = work_dir.join("Prover.toml");
    let witness_path = work_dir.join("witness.gz");
    let proof_path = work_dir.join("proof.bin");
    // Use the CRS file (bn254_g1.dat) from the CRS directory
    let crs_file = format!("{}/bn254_g1.dat", crs_path);

    tracing::info!(
        "[{}] Starting witness generation for circuit {} (node {})",
        session_id, circuit_name, node_id
    );

    // Step 1: Generate witness in MPC
    let witness_output = Command::new("co-noir")
        .arg("generate-witness")
        .arg("--circuit")
        .arg(&circuit_path)
        .arg("--input")
        .arg(&share_path)
        .arg("--protocol")
        .arg("REP3")
        .arg("--config")
        .arg(&party_config_path)
        .arg("--out")
        .arg(&witness_path)
        .output()
        .await
        .map_err(|e| format!("failed to spawn co-noir generate-witness: {}", e))?;

    if !witness_output.status.success() {
        let stderr = String::from_utf8_lossy(&witness_output.stderr);
        let stdout = String::from_utf8_lossy(&witness_output.stdout);
        return Err(format!(
            "co-noir generate-witness failed (node {}):\nstderr: {}\nstdout: {}",
            node_id, stderr, stdout
        ));
    }

    tracing::info!(
        "[{}] Witness generated, starting proof generation (node {})",
        session_id, node_id
    );

    // Step 2: Build and generate proof in MPC
    let proof_output = Command::new("co-noir")
        .arg("build-and-generate-proof")
        .arg("--circuit")
        .arg(&circuit_path)
        .arg("--witness")
        .arg(&witness_path)
        .arg("--protocol")
        .arg("REP3")
        .arg("--config")
        .arg(&party_config_path)
        .arg("--crs")
        .arg(&crs_file)
        .arg("--out")
        .arg(&proof_path)
        .output()
        .await
        .map_err(|e| format!("failed to spawn co-noir build-and-generate-proof: {}", e))?;

    if !proof_output.status.success() {
        let stderr = String::from_utf8_lossy(&proof_output.stderr);
        let stdout = String::from_utf8_lossy(&proof_output.stdout);
        return Err(format!(
            "co-noir build-and-generate-proof failed (node {}):\nstderr: {}\nstdout: {}",
            node_id, stderr, stdout
        ));
    }

    tracing::info!("[{}] Proof generated successfully (node {})", session_id, node_id);

    // Read proof bytes
    let proof_bytes = std::fs::read(&proof_path)
        .map_err(|e| format!("failed to read proof file: {}", e))?;

    Ok(proof_bytes)
}

/// Read completed proof bytes from disk.
pub fn get_proof(session: &MpcSessionState) -> Result<Vec<u8>, String> {
    let proof_path = session
        .proof_path
        .as_ref()
        .ok_or("proof not yet generated")?;

    std::fs::read(proof_path).map_err(|e| format!("failed to read proof: {}", e))
}
