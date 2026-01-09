use sha2::{Digest, Sha256};

pub fn make_receipt(node_id: &str, job_id: u64, ts: u64, work_units: u64) -> String {
    let raw = format!("{node_id}|{job_id}|{ts}|{work_units}");
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    let out = h.finalize();
    format!("rcpt:{}:{}", job_id, hex::encode(out))
}
