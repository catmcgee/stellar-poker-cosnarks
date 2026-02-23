//! Fiat–Shamir transcript for UltraHonk
//!
//! Updated to match co-noir's keccak transcript protocol:
//! - VK is pre-hashed into a single `vk_hash` field element
//! - G1 points use raw (x, y) encoding (2×32 bytes), not limb-encoded (4×32 bytes)

use crate::trace;
use crate::{
    field::Fr,
    hash::hash32,
    types::{
        G1Point, Proof, RelationParameters, Transcript, VerificationKey, CONST_PROOF_SIZE_LOG_N,
        NUMBER_OF_ALPHAS,
    },
};
use soroban_sdk::{Bytes, Env};

/// Serialize a G1 point as raw (x, y) — 2×32 bytes, matching co-noir keccak encoding.
fn push_point(buf: &mut Bytes, pt: &G1Point) {
    buf.extend_from_slice(&pt.x);
    buf.extend_from_slice(&pt.y);
}

fn split_challenge(challenge: Fr) -> (Fr, Fr) {
    let challenge_bytes = challenge.to_bytes();
    let mut low_bytes = [0u8; 32];
    low_bytes[16..].copy_from_slice(&challenge_bytes[16..]);
    let mut high_bytes = [0u8; 32];
    high_bytes[16..].copy_from_slice(&challenge_bytes[..16]);
    (Fr::from_bytes(&low_bytes), Fr::from_bytes(&high_bytes))
}

#[inline(always)]
fn hash_to_fr(bytes: &Bytes) -> Fr {
    Fr::from_bytes(&hash32(bytes))
}

fn u64_to_be32(x: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&x.to_be_bytes());
    out
}

/// Compute VK hash matching co-noir's `VerifyingKey::hash_through_transcript`.
/// keccak256([log_circuit_size, num_public_inputs, pub_inputs_offset, 28×(x, y)]) mod p
fn compute_vk_hash(env: &Env, vk: &VerificationKey) -> Fr {
    let mut buf = Bytes::new(env);
    buf.extend_from_slice(&u64_to_be32(vk.log_circuit_size));
    buf.extend_from_slice(&u64_to_be32(vk.public_inputs_size));
    buf.extend_from_slice(&u64_to_be32(vk.pub_inputs_offset));

    // 28 commitment points in BB/co-noir PrecomputedEntities order
    let commitments: [&G1Point; 28] = [
        &vk.qm,
        &vk.qc,
        &vk.ql,
        &vk.qr,
        &vk.qo,
        &vk.q4,
        &vk.q_lookup,
        &vk.q_arith,
        &vk.q_delta_range,
        &vk.q_elliptic,
        &vk.q_aux,
        &vk.q_nnf,
        &vk.q_poseidon2_external,
        &vk.q_poseidon2_internal,
        &vk.s1,
        &vk.s2,
        &vk.s3,
        &vk.s4,
        &vk.id1,
        &vk.id2,
        &vk.id3,
        &vk.id4,
        &vk.t1,
        &vk.t2,
        &vk.t3,
        &vk.t4,
        &vk.lagrange_first,
        &vk.lagrange_last,
    ];
    for pt in &commitments {
        buf.extend_from_slice(&pt.x);
        buf.extend_from_slice(&pt.y);
    }

    hash_to_fr(&buf)
}

fn generate_eta_challenge(
    env: &Env,
    proof: &Proof,
    public_inputs: &Bytes,
    vk_hash: Fr,
) -> (Fr, Fr, Fr, Fr) {
    let mut data = Bytes::new(env);

    // co-noir preamble: vk_hash first, then ALL public inputs (user + pairing), then wire commitments
    data.extend_from_slice(&vk_hash.to_bytes());
    data.append(public_inputs);
    for fr in &proof.pairing_point_object {
        data.extend_from_slice(&fr.to_bytes());
    }
    for w in &[&proof.w1, &proof.w2, &proof.w3] {
        push_point(&mut data, w);
    }

    // First challenge: no previous_challenge prepended (matches co-noir is_first_challenge=true)
    let previous_challenge = hash_to_fr(&data);
    let (eta, eta_two) = split_challenge(previous_challenge);

    // eta_three from re-hashing just the previous challenge
    let prev_bytes = Bytes::from_array(env, &previous_challenge.to_bytes());
    let previous_challenge = hash_to_fr(&prev_bytes);
    let (eta_three, _) = split_challenge(previous_challenge);

    (eta, eta_two, eta_three, previous_challenge)
}

fn generate_beta_and_gamma_challenges(
    env: &Env,
    previous_challenge: Fr,
    proof: &Proof,
) -> (Fr, Fr, Fr) {
    let mut data = Bytes::new(env);
    data.extend_from_slice(&previous_challenge.to_bytes());
    for w in &[
        &proof.lookup_read_counts,
        &proof.lookup_read_tags,
        &proof.w4,
    ] {
        push_point(&mut data, w);
    }
    let next_previous_challenge = hash_to_fr(&data);
    let (beta, gamma) = split_challenge(next_previous_challenge);
    (beta, gamma, next_previous_challenge)
}

fn generate_alpha_challenges(
    env: &Env,
    previous_challenge: Fr,
    proof: &Proof,
) -> ([Fr; NUMBER_OF_ALPHAS], Fr) {
    let mut data = Bytes::new(env);
    data.extend_from_slice(&previous_challenge.to_bytes());
    for w in &[&proof.lookup_inverses, &proof.z_perm] {
        push_point(&mut data, w);
    }
    // co-noir uses a single alpha challenge and derives powers alpha^i.
    let next_previous_challenge = hash_to_fr(&data);
    let alpha = split_challenge(next_previous_challenge).0;
    let mut alphas = [Fr::zero(); NUMBER_OF_ALPHAS];
    if NUMBER_OF_ALPHAS > 0 {
        alphas[0] = alpha;
        for i in 1..NUMBER_OF_ALPHAS {
            alphas[i] = alphas[i - 1] * alpha;
        }
    }
    (alphas, next_previous_challenge)
}

fn generate_relation_parameters_challenges(
    env: &Env,
    proof: &Proof,
    public_inputs: &Bytes,
    vk: &VerificationKey,
) -> (RelationParameters, Fr) {
    let vk_hash = compute_vk_hash(env, vk);
    trace!("vk_hash = 0x{}", hex::encode(vk_hash.to_bytes()));

    let (eta, eta_two, eta_three, previous_challenge) =
        generate_eta_challenge(env, proof, public_inputs, vk_hash);
    let (beta, gamma, next_previous_challenge) =
        generate_beta_and_gamma_challenges(env, previous_challenge, proof);
    let rp = RelationParameters {
        eta,
        eta_two,
        eta_three,
        beta,
        gamma,
        public_inputs_delta: Fr::zero(),
    };
    (rp, next_previous_challenge)
}

fn generate_gate_challenges(
    env: &Env,
    previous_challenge: Fr,
) -> ([Fr; CONST_PROOF_SIZE_LOG_N], Fr) {
    // co-noir uses one gate challenge then repeated squaring for powers.
    let next_bytes = Bytes::from_array(env, &previous_challenge.to_bytes());
    let next_previous_challenge = hash_to_fr(&next_bytes);
    let gate_challenge = split_challenge(next_previous_challenge).0;
    let mut gate_challenges = [Fr::zero(); CONST_PROOF_SIZE_LOG_N];
    if CONST_PROOF_SIZE_LOG_N > 0 {
        gate_challenges[0] = gate_challenge;
        for i in 1..CONST_PROOF_SIZE_LOG_N {
            gate_challenges[i] = gate_challenges[i - 1] * gate_challenges[i - 1];
        }
    }
    (gate_challenges, next_previous_challenge)
}

fn generate_sumcheck_challenges(
    env: &Env,
    proof: &Proof,
    previous_challenge: Fr,
    log_n: usize,
) -> ([Fr; CONST_PROOF_SIZE_LOG_N], Fr) {
    let mut next_previous_challenge = previous_challenge;
    let mut sumcheck_challenges = [Fr::zero(); CONST_PROOF_SIZE_LOG_N];
    // With Keccak transcript, co-noir does not pad rounds: only hash real log_n rounds.
    for r in 0..log_n.min(CONST_PROOF_SIZE_LOG_N) {
        let mut data = Bytes::new(env);
        data.extend_from_slice(&next_previous_challenge.to_bytes());
        for &c in proof.sumcheck_univariates[r].iter() {
            data.extend_from_slice(&c.to_bytes());
        }
        next_previous_challenge = hash_to_fr(&data);
        sumcheck_challenges[r] = split_challenge(next_previous_challenge).0;
    }
    (sumcheck_challenges, next_previous_challenge)
}

fn generate_rho_challenge(env: &Env, proof: &Proof, previous_challenge: Fr) -> (Fr, Fr) {
    let mut data = Bytes::new(env);
    data.extend_from_slice(&previous_challenge.to_bytes());
    for &e in proof.sumcheck_evaluations.iter() {
        data.extend_from_slice(&e.to_bytes());
    }
    let next_previous_challenge = hash_to_fr(&data);
    let rho = split_challenge(next_previous_challenge).0;
    (rho, next_previous_challenge)
}

fn generate_gemini_r_challenge(
    env: &Env,
    proof: &Proof,
    previous_challenge: Fr,
    log_n: usize,
) -> (Fr, Fr) {
    let mut data = Bytes::new(env);
    data.extend_from_slice(&previous_challenge.to_bytes());
    // Keccak transcript does not pad: hash only real Gemini fold commitments.
    let num_fold_comms = log_n.saturating_sub(1).min(CONST_PROOF_SIZE_LOG_N - 1);
    for pt in proof.gemini_fold_comms.iter().take(num_fold_comms) {
        push_point(&mut data, pt);
    }
    let next_previous_challenge = hash_to_fr(&data);
    let gemini_r = split_challenge(next_previous_challenge).0;
    (gemini_r, next_previous_challenge)
}

fn generate_shplonk_nu_challenge(
    env: &Env,
    proof: &Proof,
    previous_challenge: Fr,
    log_n: usize,
) -> (Fr, Fr) {
    let mut data = Bytes::new(env);
    data.extend_from_slice(&previous_challenge.to_bytes());
    // Keccak transcript does not pad: hash only real Gemini evaluations.
    for &a in proof
        .gemini_a_evaluations
        .iter()
        .take(log_n.min(CONST_PROOF_SIZE_LOG_N))
    {
        data.extend_from_slice(&a.to_bytes());
    }
    let next_previous_challenge = hash_to_fr(&data);
    let shplonk_nu = split_challenge(next_previous_challenge).0;
    (shplonk_nu, next_previous_challenge)
}

fn generate_shplonk_z_challenge(env: &Env, proof: &Proof, previous_challenge: Fr) -> (Fr, Fr) {
    let mut data = Bytes::new(env);
    data.extend_from_slice(&previous_challenge.to_bytes());
    push_point(&mut data, &proof.shplonk_q);
    let next_previous_challenge = hash_to_fr(&data);
    let shplonk_z = split_challenge(next_previous_challenge).0;
    (shplonk_z, next_previous_challenge)
}

pub fn generate_transcript(
    env: &Env,
    proof: &Proof,
    public_inputs: &Bytes,
    vk: &VerificationKey,
) -> Transcript {
    let log_n = vk.log_circuit_size as usize;
    // 1) eta/beta/gamma (uses VK hash instead of raw VK fields)
    let (rp, previous_challenge) =
        generate_relation_parameters_challenges(env, proof, public_inputs, vk);

    // 2) alphas
    let (alphas, previous_challenge) = generate_alpha_challenges(env, previous_challenge, proof);

    // 3) gate challenges
    let (gate_chals, previous_challenge) = generate_gate_challenges(env, previous_challenge);

    // 4) sumcheck challenges
    let (u_chals, previous_challenge) =
        generate_sumcheck_challenges(env, proof, previous_challenge, log_n);

    // 5) rho
    let (rho, previous_challenge) = generate_rho_challenge(env, proof, previous_challenge);

    // 6) gemini_r
    let (gemini_r, previous_challenge) =
        generate_gemini_r_challenge(env, proof, previous_challenge, log_n);

    // 7) shplonk_nu
    let (shplonk_nu, previous_challenge) =
        generate_shplonk_nu_challenge(env, proof, previous_challenge, log_n);

    // 8) shplonk_z
    let (shplonk_z, _previous_challenge) =
        generate_shplonk_z_challenge(env, proof, previous_challenge);

    trace!("===== TRANSCRIPT PARAMETERS =====");
    trace!("eta = 0x{}", hex::encode(rp.eta.to_bytes()));
    trace!("eta_two = 0x{}", hex::encode(rp.eta_two.to_bytes()));
    trace!("eta_three = 0x{}", hex::encode(rp.eta_three.to_bytes()));
    trace!("beta = 0x{}", hex::encode(rp.beta.to_bytes()));
    trace!("gamma = 0x{}", hex::encode(rp.gamma.to_bytes()));
    trace!("rho = 0x{}", hex::encode(rho.to_bytes()));
    trace!("gemini_r = 0x{}", hex::encode(gemini_r.to_bytes()));
    trace!("shplonk_nu = 0x{}", hex::encode(shplonk_nu.to_bytes()));
    trace!("shplonk_z = 0x{}", hex::encode(shplonk_z.to_bytes()));
    trace!("circuit_size = {}", vk.circuit_size);
    trace!("public_inputs_total = {}", vk.public_inputs_size);
    trace!("public_inputs_offset = {}", vk.pub_inputs_offset);
    trace!("=================================");

    Transcript {
        rel_params: rp,
        alphas,
        gate_challenges: gate_chals,
        sumcheck_u_challenges: u_chals,
        rho,
        gemini_r,
        shplonk_nu,
        shplonk_z,
    }
}
