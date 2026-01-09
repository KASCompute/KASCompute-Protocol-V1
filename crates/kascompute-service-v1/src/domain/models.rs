use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub node_id: String,
    pub public_key_hex: String,
    pub last_seen_unix: u64,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub country: Option<String>,
    pub roles: Vec<String>,          // ["node","miner"]
    pub compute_profile: Option<String>, // legacy compatibility
    pub client_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatPayload {
    pub node_id: String,
    pub public_key_hex: String,

    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub compute_profile: Option<String>, // legacy
    #[serde(default)]
    pub client_version: Option<String>,

    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
    #[serde(default)]
    pub country: Option<String>,

    // optional signature verification
    #[serde(default)]
    pub timestamp_unix: Option<u64>,
    #[serde(default)]
    pub signature_hex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub work_units: u64,
    pub status: JobStatus,
    pub assigned_node: Option<String>,
    pub created_unix: u64,
    pub updated_unix: u64,
    pub assigned_unix: Option<u64>,
    pub completed_unix: Option<u64>,
    pub lease_expires_unix: Option<u64>,
    pub is_demo: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobsSummary {
    pub pending: u64,
    pub running: u64,
    pub completed: u64,
    pub expired: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofRecord {
    pub node_id: String,
    pub job_id: u64,

    // raw work
    pub work_units: u64,
    // effective work (verified bonus applied)
    pub effective_work_units: u64,

    pub timestamp_unix: u64,

    // proof meta
    pub workload_mode: String, // "sim" | "hash"
    pub elapsed_ms: u64,
    pub result_hash: Option<String>,
    pub client_version: Option<String>,
    pub receipt: String,

    // optional verification info
    pub signature_verified: bool,
}

#[derive(Debug, Deserialize)]
pub struct NextJobRequest {
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NextJobResponse {
    pub job: Option<JobLease>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobLease {
    pub id: u64,
    pub work_units: u64,
    pub lease_expires_unix: u64,
}

#[derive(Debug, Deserialize)]
pub struct ProofSubmitRequest {
    pub node_id: String,
    pub work_units: u64,

    #[serde(default)]
    pub workload_mode: Option<String>,
    #[serde(default)]
    pub elapsed_ms: Option<u64>,
    #[serde(default)]
    pub result_hash: Option<String>,
    #[serde(default)]
    pub client_version: Option<String>,

    // optional signature fields (recommended)
    #[serde(default)]
    pub timestamp_unix: Option<u64>,
    #[serde(default)]
    pub signature_hex: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeMiningStats {
    pub node_id: String,
    pub total_mined_nano: u64,
    pub last_block_reward_nano: u64,
    pub hashrate_share_pct: f64,
    pub cumulative_work_units: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MiningStats {
    pub block_height: u64,
    pub current_block_reward_kct: f64,
    pub current_block_reward_nano: u64,
    pub month_index: u64,
    pub total_emitted_nano: u64,
    pub per_node: Vec<NodeMiningStats>,
    pub timestamp: u64,
    pub reward_window_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStats {
    pub node_id: String,
    pub first_seen_unix: u64,
    pub last_seen_unix: u64,
    pub total_effective_work_units: u64,
    pub verified_work_units: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentJobView {
    pub id: u64,
    pub status: JobStatus,
    pub work_units: u64,
    pub assigned_node: Option<String>,
    pub created_unix: u64,
    pub updated_unix: u64,
    pub assigned_unix: Option<u64>,
    pub completed_unix: Option<u64>,
    pub lease_expires_unix: Option<u64>,
    pub workload_mode: Option<String>,
    pub elapsed_ms: Option<u64>,
    pub result_hash: Option<String>,
    pub client_version: Option<String>,
    pub receipt: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RewardView {
    pub node_id: String,
    pub effective_work_units: u64,
    pub verified_work_units: u64,
    pub share: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DemoStatus {
    pub enabled: bool,
    pub demo_nodes: usize,
    pub demo_jobs: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub window_sec: u64,
    pub active_nodes_90s: usize,
    pub active_miners_90s: usize,
    pub jobs_completed_window: usize,
    pub jobs_per_min: f64,
    pub avg_job_ms: u64,
    pub proofs_window: usize,
    pub timestamp: u64,
}
