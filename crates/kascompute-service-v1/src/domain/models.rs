use serde::{Deserialize, Serialize};

// =========================
// Core Node / Heartbeat
// =========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub node_id: String,
    pub public_key_hex: String,

    /// Global last seen (any role heartbeat)
    pub last_seen_unix: u64,

    /// Role-specific last seen (future-proof: node vs miner)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_node_unix: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_miner_unix: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,

    pub roles: Vec<String>,              // ["node","miner"]
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

// =========================
// Jobs
// =========================

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

// =========================
// Proofs (v1.1)
// =========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofRecord {
    /// Coordinator / distributor identity (job was leased to this id)
    pub node_id: String,

    /// NEW (v1.1): Worker/miner identity (always stored as concrete String)
    pub miner_id: String,

    pub job_id: u64,

    // raw work
    pub work_units: u64,

    // effective work (verified bonus applied)
    pub effective_work_units: u64,

    // NEW (v1.1): Compute Units (CU) – future-proof for AI/rendering/batch
    // v1.1 default: CU = effective_work_units
    pub compute_units: u64,

    // NEW (v1.1): prevents double counting in reward windows
    pub rewarded_block: Option<u64>,

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
pub struct ProofSubmitRequest {
    pub node_id: String,
    pub work_units: u64,

    /// NEW (v1.1): miner/worker identity (optional for backward compatibility)
    /// If not provided, server will fallback to node_id.
    #[serde(default)]
    pub miner_id: Option<String>,

    #[serde(default)]
    pub workload_mode: Option<String>,
    #[serde(default)]
    pub elapsed_ms: Option<u64>,
    #[serde(default)]
    pub result_hash: Option<String>,
    #[serde(default)]
    pub client_version: Option<String>,

    #[serde(default, alias = "ts")]
    pub timestamp_unix: Option<u64>,

    #[serde(default, alias = "signature")]
    pub signature_hex: Option<String>,

    #[serde(default, alias = "proof_hash")]
    pub proof_hash_hex: Option<String>,

    #[serde(default)]
    pub public_key_hex: Option<String>,
}

// =========================
// Mining / Rewards (compat + v1.1)
// =========================

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
    /// Backward compatible field name.
    /// v1.1 semantics: this contains the miner_id (worker identity).
    pub node_id: String,
    pub effective_work_units: u64,
    pub verified_work_units: u64,
    pub share: f64,
}

// =========================
// Miner Rewards / Ledger (v1.1)
// =========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardLedgerEntry {
    pub block_height: u64,
    pub timestamp_unix: u64,

    /// Worker identity receiving rewards
    pub miner_id: String,

    /// Amount credited for this block (nanoKCT)
    pub amount_nano: u64,

    /// Share in this block (0..1)
    pub share: f64,

    /// Accounting weight used (v1.1: sum of compute_units in window)
    pub compute_units: u64,

    /// How many proofs contributed for this miner in the block window
    pub proofs_count: usize,

    /// Free text ("window_payout", etc.)
    pub reason: String,
}

// =========================
// Node Rewards / Ledger (v1.1+)
// =========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRewardLedgerEntry {
    pub block_height: u64,
    pub timestamp_unix: u64,

    /// Coordinator node receiving rewards (20% pool)
    pub node_id: String,

    /// Amount credited for this block (nanoKCT)
    pub amount_nano: u64,

    /// Share in this block (0..1)
    pub share: f64,

    /// Accounting weight used (v1.1: sum of compute_units in window)
    pub compute_units: u64,

    /// How many proofs contributed for this node in the block window
    pub proofs_count: usize,

    /// Free text ("window_payout", etc.)
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MinerBalanceView {
    pub miner_id: String,
    pub total_mined_nano: u64,
    pub last_block_reward_nano: u64,
}

// Node rewards balance view (20% pool payouts)
#[derive(Debug, Clone, Serialize)]
pub struct NodeBalanceView {
    pub node_id: String,
    pub total_mined_nano: u64,
    pub last_block_reward_nano: u64,
}

// =========================
// Debug / Demo
// =========================

#[derive(Debug, Clone, Serialize)]
pub struct DemoStatus {
    pub enabled: bool,
    pub demo_nodes: usize,
    pub demo_jobs: usize,
}

// =========================
// Metrics
// =========================

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

// =========================
// ComputeDAG (Protocol V1)
// =========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeDagSubmitRequest {
    pub spec: crate::util::compute_dag::DagSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeDagSubmitResponse {
    pub dag_id: String,
    pub dag_root_hash: String,
    pub tasks_total: usize,
}

/// Create a new execution run for an existing DAG spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeDagRunCreateResponse {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DagTaskStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTask {
    /// Run instance id (created when starting a run)
    pub run_id: String,

    /// DAG definition id (hash of canonical DAG spec)
    pub dag_id: String,

    /// Stable task id inside a DAG
    pub task_id: String,

    /// Deterministic hash binding task to dag_root_hash + task fields
    pub task_hash: String,

    pub work_units: u64,
    pub task_type: String,
    pub deps: Vec<String>, // task_ids

    pub status: DagTaskStatus,

    pub assigned_node: Option<String>,
    pub assigned_unix: Option<u64>,
    pub lease_expires_unix: Option<u64>,
    pub completed_unix: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeDagView {
    pub dag_id: String,
    pub dag_root_hash: String,
    pub name: String,
    pub created_unix: u64,
    pub tasks_total: usize,
    pub tasks_completed: usize,
    pub tasks_ready: usize,
    pub tasks_running: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeDagRunView {
    pub run_id: String,
    pub dag_id: String,
    pub dag_root_hash: String,
    pub name: String,
    pub created_unix: u64,
    pub tasks_total: usize,
    pub tasks_ready: usize,
    pub tasks_running: usize,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNextTaskRequest {
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNextTaskResponse {
    pub task: Option<DagTaskLease>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTaskLease {
    pub run_id: String,
    pub dag_id: String,
    pub task_id: String,
    pub task_hash: String,
    pub work_units: u64,
    pub task_type: String,
    pub lease_expires_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTaskProofSubmitRequest {
    pub node_id: String,
    pub work_units: u64,
    pub elapsed_ms: u64,
    pub client_version: Option<String>,

    #[serde(default, alias = "ts")]
    pub timestamp_unix: Option<u64>,

    #[serde(default, alias = "signature")]
    pub signature_hex: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTaskProofSubmitResponse {
    pub status: String,
    pub run_id: String,
    pub task_id: String,
    pub unlocked_ready: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTaskLeaseRenewRequest {
    pub node_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTaskLeaseRenewResponse {
    pub run_id: String,
    pub task_id: String,
    pub lease_expires_unix: u64,
}
