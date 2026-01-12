use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

pub fn make_receipt(node_id: &str, job_id: u64, ts: u64, work_units: u64) -> String {
    // Stable receipt hash
    let msg = format!("{node_id}|{job_id}|{ts}|{work_units}");
    let mut h = Sha256::new();
    h.update(msg.as_bytes());
    let hex = hex::encode(h.finalize());
    format!("rcpt:{job_id}:{hex}")
}

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
