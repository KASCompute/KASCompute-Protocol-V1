use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};

pub fn make_receipt(node_id: &str, job_id: u64, ts: u64, work_units: u64) -> String {
    // Stable receipt hash
    let msg = format!("{node_id}|{job_id}|{ts}|{work_units}");
    let mut h = Sha256::new();
    h.update(msg.as_bytes());
    let hex = hex::encode(h.finalize());
    format!("rcpt:{job_id}:{hex}")
}

/// Legacy/helper: verifies signature over raw message bytes (e.g. "pf|...").
/// Keep this for backwards compatibility / debugging.
pub fn verify_ed25519_signature_hex(public_key_hex: &str, signature_hex: &str, message: &str) -> bool {
    let pk = match hex::decode(public_key_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };
    if pk.len() != 32 {
        return false;
    }
    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(&pk);

    let vk = match VerifyingKey::from_bytes(&pk_arr) {
        Ok(k) => k,
        Err(_) => return false,
    };

    let sig = match hex::decode(signature_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };
    if sig.len() != 64 {
        return false;
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig);

    let signature = Signature::from_bytes(&sig_arr);
    vk.verify(message.as_bytes(), &signature).is_ok()
}

/* =========================
   Protocol V1 Proof Verify (Launcher-compatible)
   - Miner signs: ed25519( sha256( serde_json::to_vec(payload) ) )
   ========================= */

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofPayloadV1 {
    pub node_id: String,
    pub job_id: u64,
    pub work_units: u64,
    pub workload_mode: String,
    pub elapsed_ms: u64,
    pub client_version: String,
    pub ts: u64,
}

pub fn verify_proof_signature_v1(public_key_hex: &str, signature_hex: &str, payload: &ProofPayloadV1) -> bool {
    // pubkey
    let pk = match hex::decode(public_key_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };
    if pk.len() != 32 {
        return false;
    }
    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(&pk);

    let vk = match VerifyingKey::from_bytes(&pk_arr) {
        Ok(k) => k,
        Err(_) => return false,
    };

    // signature
    let sig = match hex::decode(signature_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };
    if sig.len() != 64 {
        return false;
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig);
    let signature = Signature::from_bytes(&sig_arr);

    // canonical bytes
    let payload_bytes = match serde_json::to_vec(payload) {
        Ok(v) => v,
        Err(_) => return false,
    };

    // sha256(payload_bytes)
    let hash = Sha256::digest(&payload_bytes);

    // verify signature over hash BYTES
    vk.verify(hash.as_slice(), &signature).is_ok()
}
