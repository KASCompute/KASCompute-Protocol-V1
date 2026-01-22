use std::collections::{HashMap, HashSet, VecDeque};
use std::net::IpAddr;
use std::sync::Arc;

use rand::Rng;
use tokio::sync::RwLock;

use crate::domain::models::*;
use crate::util::compute_dag::{dag_id_hex, dag_root_hash_hex, task_hash_hex, validate_dag, DagSpec};
use crate::util::geo::geo_lookup;
use crate::util::receipt::{
    make_receipt, verify_ed25519_signature_hex, verify_proof_signature_v1, ProofPayloadV1,
};
use crate::util::time::{months_since, now_unix};

// ============================================================================
// Protocol Constants (Demo/Testnet)
// ============================================================================

pub const NANO: u64 = 100_000_000; // 1 KCT = 100_000_000 nanoKCT
pub const START_REWARD_KCT: f64 = 200.0;
pub const MONTHLY_DECAY: f64 = 0.99; // -1% per month
pub const VERIFIED_BONUS_MULT: f64 = 1.10;

pub const MIN_UPTIME_FOR_REWARDS_SEC: u64 = 60;
pub const REWARD_WINDOW_SEC: u64 = 300;
pub const ACTIVE_WINDOW_SEC: u64 = 90;

pub const JOB_SCHEDULE_EVERY_SEC: u64 = 10;
pub const JOB_LEASE_SEC: u64 = 60;

// ComputeDAG task lease (independent from normal jobs)
pub const DAG_TASK_LEASE_SEC: u64 = 90;

// ============================================================================
// Scheduler Policy (Protocol-v1, clean production semantics)
// ============================================================================

/// Heartbeat TTL for "online" miners used by the job-pool policy.
/// Keep this aligned with your launcher heartbeat cadence + dashboard TTL.
pub const NODE_ONLINE_TTL_SEC: u64 = 180;

/// Maintain at least this many pending jobs (pool floor).
pub const JOB_POOL_MIN: usize = 10;

/// Hard cap to avoid unbounded memory growth.
pub const JOB_POOL_MAX: usize = 200;

/// Pool target factor: pending_jobs ~= online_miners * JOBS_PER_MINER
pub const JOBS_PER_MINER: usize = 2;

// ============================================================================
// AppState + InnerState
// ============================================================================

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<RwLock<InnerState>>,
}

pub struct InnerState {
    // ------------------------------------------------------------------------
    // Core network state
    // ------------------------------------------------------------------------
    pub nodes: HashMap<String, Node>,
    pub jobs: HashMap<u64, Job>,
    pub proofs: Vec<ProofRecord>,

    // ------------------------------------------------------------------------
    // Chain / emission state
    // ------------------------------------------------------------------------
    pub next_job_id: u64,
    pub genesis_time: u64,
    pub block_height: u64,
    pub total_emitted_nano: u64,

    // ------------------------------------------------------------------------
    // Accounting / stats
    // ------------------------------------------------------------------------
    pub node_rewards: HashMap<String, u64>,
    pub node_last_block_reward: HashMap<String, u64>,
    pub node_cumulative_work: HashMap<String, u64>,
    pub node_stats: HashMap<String, NodeStats>,

    // ------------------------------------------------------------------------
    // Demo mode
    // ------------------------------------------------------------------------
    pub demo_running: bool,

    // ------------------------------------------------------------------------
    // ComputeDAG: Spec Registry + Runs (Mainnet-ready Scheduling Model)
    // ------------------------------------------------------------------------
    pub dag_meta: HashMap<String, DagMeta>,      // dag_id -> meta
    pub dag_specs: HashMap<String, DagSpec>,     // dag_id -> immutable spec
    pub next_run_seq: u64,                       // monotonic run counter
    pub dag_runs: HashMap<String, DagRunState>,  // run_id -> execution state
}

#[derive(Debug, Clone)]
pub struct DagMeta {
    pub dag_id: String,
    pub dag_root_hash: String,
    pub name: String,
    pub created_unix: u64,
    pub tasks_total: usize,
}

/// Internal run state (kept in-memory for testnet/demo).
/// Mainnet later: persist this + add durability/replay.
#[derive(Debug, Clone)]
pub struct DagRunState {
    pub run_id: String,
    pub dag_id: String,
    pub dag_root_hash: String,
    pub name: String,
    pub created_unix: u64,

    pub tasks_total: usize,

    /// task_id -> task model (includes lease + status)
    pub tasks: HashMap<String, DagTask>,

    /// task_id -> list of dependent task_ids
    pub dependents: HashMap<String, Vec<String>>,

    /// task_id -> remaining dependency count (fast scheduling)
    pub remaining_deps: HashMap<String, usize>,

    /// FIFO queue for ready tasks
    pub ready_q: VecDeque<String>,
}

// ============================================================================
// AppState Implementation
// ============================================================================

impl AppState {
    pub fn new(genesis_time: u64) -> Self {
        let inner = InnerState {
            nodes: HashMap::new(),
            jobs: HashMap::new(),
            proofs: Vec::new(),

            next_job_id: 1,
            genesis_time,
            block_height: 0,
            total_emitted_nano: 0,

            node_rewards: HashMap::new(),
            node_last_block_reward: HashMap::new(),
            node_cumulative_work: HashMap::new(),
            node_stats: HashMap::new(),

            demo_running: false,

            dag_meta: HashMap::new(),
            dag_specs: HashMap::new(),
            next_run_seq: 1,
            dag_runs: HashMap::new(),
        };

        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    // =========================================================================
    // Protocol-v1 Job Pool (clean production scheduler)
    // =========================================================================

    /// Ensure a stable pool of Pending jobs exists based on currently online miners.
    /// This avoids "job: null" when miners are online, without injecting fake jobs
    /// inside the /jobs/next route itself.
    pub async fn ensure_job_pool(&self) {
        let now = now_unix();
        let mut s = self.inner.write().await;

        // Online miners = nodes fresh within TTL + role contains "miner"
        let online_miners = s
            .nodes
            .values()
            .filter(|n| now.saturating_sub(n.last_seen_unix) <= NODE_ONLINE_TTL_SEC)
            .filter(|n| n.roles.iter().any(|r| r == "miner"))
            .count();

        let pending = s
            .jobs
            .values()
            .filter(|j| matches!(j.status, JobStatus::Pending))
            .count();

        // target pool size
        let mut target = online_miners.saturating_mul(JOBS_PER_MINER);
        if target < JOB_POOL_MIN {
            target = JOB_POOL_MIN;
        }
        if target > JOB_POOL_MAX {
            target = JOB_POOL_MAX;
        }

        if pending >= target {
            return;
        }

        let to_create = target - pending;
        for _ in 0..to_create {
            let wu: u64 = rand::thread_rng().gen_range(500_000..=5_000_000);
            let id = s.next_job_id;
            s.next_job_id += 1;

            let ts = now_unix();
            s.jobs.insert(
                id,
                Job {
                    id,
                    work_units: wu,
                    status: JobStatus::Pending,
                    assigned_node: None,
                    created_unix: ts,
                    updated_unix: ts,
                    assigned_unix: None,
                    completed_unix: None,
                    lease_expires_unix: None,
                    is_demo: false,
                },
            );
        }
    }

    // =========================================================================
    // ComputeDAG: Task Lease Renew (unchanged)
    // =========================================================================

    pub async fn renew_dag_task_lease(
        &self,
        run_id: &str,
        task_id: &str,
        node_id: &str,
    ) -> Result<u64, &'static str> {
        let now = now_unix();
        let mut s = self.inner.write().await;

        // Optional aber mainnet-sinnvoll: node muss existieren
        if !s.nodes.contains_key(node_id) {
            return Err("node_not_found");
        }

        let run = s.dag_runs.get_mut(run_id).ok_or("run_not_found")?;

        // Wichtig: konsistent mit next/proof
        Self::expire_dag_task_leases_locked(run, now);

        let t = run.tasks.get_mut(task_id).ok_or("task_not_found")?;

        if t.status != DagTaskStatus::Running {
            return Err("task_not_running");
        }

        if t.assigned_node.as_deref() != Some(node_id) {
            return Err("task_wrong_node");
        }

        let exp = t.lease_expires_unix.ok_or("lease_missing")?;
        if now >= exp {
            return Err("lease_expired");
        }

        let new_exp = now + DAG_TASK_LEASE_SEC;
        t.lease_expires_unix = Some(new_exp);

        Ok(new_exp)
    }

    // =========================================================================
    // Demo Mode (Development Utilities)
    // =========================================================================

    /// Spawn demo nodes + demo jobs for UI testing.
    /// Safe for testnet/dev only (gated behind /debug routes).
    pub async fn demo_spawn(&self, nodes: usize, jobs: usize) {
        let now = now_unix();
        let mut s = self.inner.write().await;
        s.demo_running = true;

        // demo nodes
        for i in 0..nodes {
            let node_id = format!("demo-node-{:02}", i + 1);
            let lat = 48.0 + (i as f64 * 0.18);
            let lon = 11.0 + (i as f64 * 0.22);

            s.nodes.insert(
                node_id.clone(),
                Node {
                    node_id: node_id.clone(),
                    public_key_hex: format!("demo{:02}deadbeef", i + 1),
                    last_seen_unix: now,
                    latitude: Some(lat),
                    longitude: Some(lon),
                    country: Some("DEMO".to_string()),
                    roles: vec!["node".to_string(), "miner".to_string()],
                    compute_profile: Some("sim".to_string()),
                    client_version: Some("demo".to_string()),
                },
            );

            s.node_rewards.entry(node_id.clone()).or_insert(0);
            s.node_last_block_reward.entry(node_id.clone()).or_insert(0);
            s.node_cumulative_work.entry(node_id.clone()).or_insert(0);

            s.node_stats.entry(node_id.clone()).or_insert(NodeStats {
                node_id: node_id.clone(),
                first_seen_unix: now.saturating_sub(MIN_UPTIME_FOR_REWARDS_SEC + 10),
                last_seen_unix: now,
                total_effective_work_units: 0,
                verified_work_units: 0,
            });
        }

        // demo jobs
        for _ in 0..jobs {
            let wu: u64 = rand::thread_rng().gen_range(500_000..=5_000_000);
            let id = s.next_job_id;
            s.next_job_id += 1;

            s.jobs.insert(
                id,
                Job {
                    id,
                    work_units: wu,
                    status: JobStatus::Pending,
                    assigned_node: None,
                    created_unix: now,
                    updated_unix: now,
                    assigned_unix: None,
                    completed_unix: None,
                    lease_expires_unix: None,
                    is_demo: true,
                },
            );
        }
    }

    /// Clear all demo artifacts.
    pub async fn demo_clear(&self) {
        let mut s = self.inner.write().await;
        s.demo_running = false;

        s.nodes.retain(|k, _| !k.starts_with("demo-"));
        s.node_rewards.retain(|k, _| !k.starts_with("demo-"));
        s.node_last_block_reward.retain(|k, _| !k.starts_with("demo-"));
        s.node_cumulative_work.retain(|k, _| !k.starts_with("demo-"));
        s.node_stats.retain(|k, _| !k.starts_with("demo-"));
        s.proofs.retain(|p| !p.node_id.starts_with("demo-"));
        s.jobs.retain(|_, j| !j.is_demo);
    }

    /// Demo status for dashboards and debug endpoints.
    pub async fn demo_status(&self) -> DemoStatus {
        let s = self.inner.read().await;
        let demo_nodes = s.nodes.keys().filter(|k| k.starts_with("demo-")).count();
        let demo_jobs = s.jobs.values().filter(|j| j.is_demo).count();

        DemoStatus {
            enabled: s.demo_running,
            demo_nodes,
            demo_jobs,
        }
    }

    // =========================================================================
    // Read Helpers
    // =========================================================================

    pub async fn list_nodes(&self) -> Vec<Node> {
        let s = self.inner.read().await;
        let mut nodes: Vec<Node> = s.nodes.values().cloned().collect();
        nodes.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        nodes
    }

    pub async fn list_jobs(&self) -> Vec<Job> {
        let s = self.inner.read().await;
        let mut v: Vec<Job> = s.jobs.values().cloned().collect();
        v.sort_by_key(|j| j.id);
        v
    }

    pub async fn summarize_jobs(&self) -> JobsSummary {
        let s = self.inner.read().await;
        let mut p = 0;
        let mut r = 0;
        let mut c = 0;
        let mut e = 0;

        for j in s.jobs.values() {
            match j.status {
                JobStatus::Pending => p += 1,
                JobStatus::Running => r += 1,
                JobStatus::Completed => c += 1,
                JobStatus::Expired => e += 1,
            }
        }

        JobsSummary {
            pending: p,
            running: r,
            completed: c,
            expired: e,
            total: p + r + c + e,
        }
    }

    pub async fn list_recent_proofs(&self, limit: usize) -> Vec<ProofRecord> {
        let s = self.inner.read().await;
        let mut v = s.proofs.clone();
        v.sort_by(|a, b| b.timestamp_unix.cmp(&a.timestamp_unix));
        if v.len() > limit {
            v.truncate(limit);
        }
        v
    }

    pub async fn list_recent_jobs_with_proofs(&self, limit: usize) -> Vec<RecentJobView> {
        let s = self.inner.read().await;

        let mut jobs: Vec<Job> = s.jobs.values().cloned().collect();
        jobs.sort_by(|a, b| b.updated_unix.cmp(&a.updated_unix));
        if jobs.len() > limit {
            jobs.truncate(limit);
        }

        jobs.into_iter()
            .map(|j| {
                let proof = s.proofs.iter().rev().find(|p| p.job_id == j.id);

                RecentJobView {
                    id: j.id,
                    status: j.status.clone(),
                    work_units: j.work_units,
                    assigned_node: j.assigned_node.clone(),
                    created_unix: j.created_unix,
                    updated_unix: j.updated_unix,
                    assigned_unix: j.assigned_unix,
                    completed_unix: j.completed_unix,
                    lease_expires_unix: j.lease_expires_unix,
                    workload_mode: proof.map(|p| p.workload_mode.clone()),
                    elapsed_ms: proof.map(|p| p.elapsed_ms),
                    result_hash: proof.and_then(|p| p.result_hash.clone()),
                    client_version: proof.and_then(|p| p.client_version.clone()),
                    receipt: proof.map(|p| p.receipt.clone()),
                }
            })
            .collect()
    }

    pub async fn rewards_leaderboard(&self) -> Vec<RewardView> {
        let s = self.inner.read().await;

        let total: u64 = s
            .node_stats
            .values()
            .map(|n| n.total_effective_work_units)
            .sum();

        let mut out: Vec<RewardView> = s
            .node_stats
            .values()
            .map(|n| RewardView {
                node_id: n.node_id.clone(),
                effective_work_units: n.total_effective_work_units,
                verified_work_units: n.verified_work_units,
                share: if total > 0 {
                    n.total_effective_work_units as f64 / total as f64
                } else {
                    0.0
                },
            })
            .collect();

        out.sort_by(|a, b| b.effective_work_units.cmp(&a.effective_work_units));
        out
    }

    // =========================================================================
    // Heartbeats
    // =========================================================================

    pub async fn upsert_node(&self, mut payload: HeartbeatPayload, client_ip: IpAddr) {
        let now = now_unix();

        // Geo enrichment (best-effort)
        if payload.latitude.is_none() || payload.longitude.is_none() {
            if let Some((lat, lon, country)) = geo_lookup(client_ip).await {
                if payload.latitude.is_none() {
                    payload.latitude = Some(lat);
                }
                if payload.longitude.is_none() {
                    payload.longitude = Some(lon);
                }
                if payload.country.is_none() {
                    payload.country = country;
                }
            }
        }

        let roles = if payload.roles.is_empty() {
            vec!["node".to_string()]
        } else {
            payload.roles.clone()
        };

        {
            // IMPORTANT: keep lock scope minimal (no await inside write lock)
            let mut s = self.inner.write().await;
            let node_id = payload.node_id.clone();

            let entry = s.nodes.entry(node_id.clone()).or_insert(Node {
                node_id: node_id.clone(),
                public_key_hex: payload.public_key_hex.clone(),
                last_seen_unix: now,
                latitude: payload.latitude,
                longitude: payload.longitude,
                country: payload.country.clone(),
                roles: roles.clone(),
                compute_profile: payload.compute_profile.clone(),
                client_version: payload.client_version.clone(),
            });

            entry.last_seen_unix = now;
            entry.public_key_hex = payload.public_key_hex.clone();
            entry.roles = roles;
            entry.compute_profile = payload.compute_profile.clone();
            entry.client_version = payload.client_version.clone();

            if payload.latitude.is_some() {
                entry.latitude = payload.latitude;
            }
            if payload.longitude.is_some() {
                entry.longitude = payload.longitude;
            }
            if payload.country.is_some() {
                entry.country = payload.country;
            }

            // Ensure accounting maps exist
            s.node_rewards.entry(node_id.clone()).or_insert(0);
            s.node_last_block_reward.entry(node_id.clone()).or_insert(0);
            s.node_cumulative_work.entry(node_id.clone()).or_insert(0);

            // node_stats init/update
            let st = s.node_stats.entry(node_id.clone()).or_insert(NodeStats {
                node_id: node_id.clone(),
                first_seen_unix: now,
                last_seen_unix: now,
                total_effective_work_units: 0,
                verified_work_units: 0,
            });
            st.last_seen_unix = now;
        }

        // Trigger pool after heartbeat (clean, no lock held)
        self.ensure_job_pool().await;
    }

    // =========================================================================
    // Jobs / Scheduler
    // =========================================================================

    pub async fn create_scheduled_job(&self, is_demo: bool) -> u64 {
        let wu: u64 = rand::thread_rng().gen_range(500_000..=5_000_000);

        let mut s = self.inner.write().await;
        let id = s.next_job_id;
        s.next_job_id += 1;

        let now = now_unix();
        s.jobs.insert(
            id,
            Job {
                id,
                work_units: wu,
                status: JobStatus::Pending,
                assigned_node: None,
                created_unix: now,
                updated_unix: now,
                assigned_unix: None,
                completed_unix: None,
                lease_expires_unix: None,
                is_demo,
            },
        );

        id
    }

    fn expire_leases_locked(s: &mut InnerState, now: u64) {
        for j in s.jobs.values_mut() {
            if matches!(j.status, JobStatus::Running) {
                if let Some(exp) = j.lease_expires_unix {
                    if now >= exp {
                        j.status = JobStatus::Expired;
                        j.updated_unix = now;
                    }
                }
            }
        }
    }

    pub async fn assign_job(&self, node_id: &str) -> Option<JobLease> {
        // Failsafe: ensure pool exists even if caller never heartbeats (still requires node existence)
        self.ensure_job_pool().await;

        let now = now_unix();
        let mut s = self.inner.write().await;

        Self::expire_leases_locked(&mut s, now);

        if !s.nodes.contains_key(node_id) {
            return None;
        }

        // Pick first pending
        let mut picked: Option<u64> = None;
        for (id, job) in s.jobs.iter() {
            if matches!(job.status, JobStatus::Pending) {
                picked = Some(*id);
                break;
            }
        }

        let job_id = picked?;
        let job = s.jobs.get_mut(&job_id).unwrap();

        job.status = JobStatus::Running;
        job.assigned_node = Some(node_id.to_string());
        job.assigned_unix = Some(now);
        job.lease_expires_unix = Some(now + JOB_LEASE_SEC);
        job.updated_unix = now;

        Some(JobLease {
            id: job.id,
            work_units: job.work_units,
            lease_expires_unix: job.lease_expires_unix.unwrap_or(now + JOB_LEASE_SEC),
        })
    }

    // =========================================================================
    // Proof Submit + Signature Verification
    // =========================================================================

    pub async fn complete_job(
        &self,
        job_id: u64,
        proof: ProofSubmitRequest,
    ) -> Result<ProofRecord, &'static str> {
        let ts = now_unix();
        let mut s = self.inner.write().await;

        // expire old leases first
        Self::expire_leases_locked(&mut s, ts);

        let job = s.jobs.get_mut(&job_id).ok_or("job_not_found")?;
        if !matches!(job.status, JobStatus::Running) {
            return Err("job_not_running");
        }
        if job.assigned_node.as_ref() != Some(&proof.node_id) {
            return Err("job_wrong_node");
        }

        // finalize job
        job.status = JobStatus::Completed;
        job.updated_unix = ts;
        job.completed_unix = Some(ts);

        // workload mode is informational only
        let mode = proof
            .workload_mode
            .clone()
            .unwrap_or_else(|| "sim".to_string());

        // fallback for elapsed_ms
        let elapsed_ms = proof.elapsed_ms.unwrap_or_else(|| {
            if let Some(a) = job.assigned_unix {
                (ts.saturating_sub(a)) * 1000
            } else {
                0
            }
        });

        // signature verification
        let mut sig_ok = false;
        let ts_msg = proof.timestamp_unix.unwrap_or(ts);

        // optional replay protection ±60s
        if let Some(ts_sent) = proof.timestamp_unix {
            if ts.abs_diff(ts_sent) > 60 {
                // keep sig_ok false
            }
        }

        if let (Some(sig_hex), Some(node)) =
            (proof.signature_hex.clone(), s.nodes.get(&proof.node_id))
        {
            // V1 verify: ed25519( sha256( json(payload) ) )
            let payload = ProofPayloadV1 {
                node_id: proof.node_id.clone(),
                job_id,
                work_units: proof.work_units,
                workload_mode: mode.clone(),
                elapsed_ms,
                client_version: proof
                    .client_version
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                ts: ts_msg,
            };

            sig_ok = verify_proof_signature_v1(&node.public_key_hex, &sig_hex, &payload);

            // fallback legacy
            if !sig_ok {
                let legacy_msg = format!(
                    "pf|{}|{}|{}|{}",
                    proof.node_id, job_id, proof.work_units, ts_msg
                );
                sig_ok = verify_ed25519_signature_hex(&node.public_key_hex, &sig_hex, &legacy_msg);
            }
        }

        // strict verify only if explicitly declared
        if proof.client_version.as_deref() == Some("protocol-v1") && !sig_ok {
            return Err("invalid_signature");
        }

        // effective work: bonus only if signature verified
        let effective_wu = if sig_ok {
            (proof.work_units as f64 * VERIFIED_BONUS_MULT) as u64
        } else {
            proof.work_units
        };

        let receipt = make_receipt(&proof.node_id, job_id, ts, proof.work_units);

        let record = ProofRecord {
            node_id: proof.node_id.clone(),
            job_id,
            work_units: proof.work_units,
            effective_work_units: effective_wu,
            timestamp_unix: ts,
            workload_mode: mode,
            elapsed_ms,
            result_hash: proof.result_hash.clone(),
            client_version: proof.client_version.clone(),
            receipt: receipt.clone(),
            signature_verified: sig_ok,
        };

        s.proofs.push(record.clone());

        // cumulative work tracking (raw)
        *s.node_cumulative_work
            .entry(proof.node_id.clone())
            .or_insert(0) += proof.work_units;

        // node stats
        let st = s.node_stats.entry(proof.node_id.clone()).or_insert(NodeStats {
            node_id: proof.node_id.clone(),
            first_seen_unix: ts,
            last_seen_unix: ts,
            total_effective_work_units: 0,
            verified_work_units: 0,
        });

        st.last_seen_unix = ts;
        st.total_effective_work_units += effective_wu;

        if sig_ok {
            st.verified_work_units += proof.work_units;
        }

        Ok(record)
    }

    // =========================================================================
    // ComputeDAG: Spec Registry
    // =========================================================================

    /// Submit an immutable DAG spec (blueprint).
    /// Returns deterministic identifiers derived from canonical spec hashing.
    pub async fn submit_dag_spec(
        &self,
        spec: DagSpec,
    ) -> Result<ComputeDagSubmitResponse, &'static str> {
        validate_dag(&spec)?;

        let dag_id = dag_id_hex(&spec);
        let root = dag_root_hash_hex(&spec);
        let now = now_unix();

        let mut s = self.inner.write().await;

        if s.dag_specs.contains_key(&dag_id) {
            return Err("dag_already_exists");
        }

        s.dag_specs.insert(dag_id.clone(), spec.clone());
        s.dag_meta.insert(
            dag_id.clone(),
            DagMeta {
                dag_id: dag_id.clone(),
                dag_root_hash: root.clone(),
                name: spec.name.clone(),
                created_unix: now,
                tasks_total: spec.nodes.len(),
            },
        );

        Ok(ComputeDagSubmitResponse {
            dag_id,
            dag_root_hash: root,
            tasks_total: spec.nodes.len(),
        })
    }

    pub async fn list_dags(&self) -> Vec<ComputeDagView> {
        let s = self.inner.read().await;
        let mut out: Vec<ComputeDagView> = s
            .dag_meta
            .keys()
            .map(|dag_id| Self::compute_dag_view_locked(&s, dag_id))
            .collect();

        out.sort_by(|a, b| a.dag_id.cmp(&b.dag_id));
        out
    }

    pub async fn get_dag_view(&self, dag_id: &str) -> Option<ComputeDagView> {
        let s = self.inner.read().await;
        if !s.dag_meta.contains_key(dag_id) {
            return None;
        }
        Some(Self::compute_dag_view_locked(&s, dag_id))
    }

    fn compute_dag_view_locked(s: &InnerState, dag_id: &str) -> ComputeDagView {
        let meta = s.dag_meta.get(dag_id).expect("dag exists");

        // Aggregate status across all runs of this spec (useful for dashboards).
        let mut ready = 0usize;
        let mut running = 0usize;
        let mut completed = 0usize;

        for r in s.dag_runs.values() {
            if r.dag_id != dag_id {
                continue;
            }
            for t in r.tasks.values() {
                match t.status {
                    DagTaskStatus::Ready => ready += 1,
                    DagTaskStatus::Running => running += 1,
                    DagTaskStatus::Completed => completed += 1,
                    _ => {}
                }
            }
        }

        ComputeDagView {
            dag_id: meta.dag_id.clone(),
            dag_root_hash: meta.dag_root_hash.clone(),
            name: meta.name.clone(),
            created_unix: meta.created_unix,
            tasks_total: meta.tasks_total,
            tasks_completed: completed,
            tasks_ready: ready,
            tasks_running: running,
        }
    }

    // =========================================================================
    // ComputeDAG: Run Lifecycle
    // =========================================================================

    /// Create a new execution run for an existing DAG spec.
    pub async fn create_dag_run(&self, dag_id: &str) -> Result<String, &'static str> {
        let mut s = self.inner.write().await;

        let spec = s.dag_specs.get(dag_id).cloned().ok_or("dag_not_found")?;
        let meta = s.dag_meta.get(dag_id).cloned().ok_or("dag_not_found")?;

        let run_id = format!("run-{}-{}", dag_id, s.next_run_seq);
        s.next_run_seq += 1;

        // Build deps + dependents from edges (to <- from)
        let mut deps: HashMap<String, Vec<String>> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        for n in &spec.nodes {
            deps.insert(n.task_id.clone(), Vec::new());
            dependents.insert(n.task_id.clone(), Vec::new());
        }
        for e in &spec.edges {
            deps.entry(e.to.clone()).or_default().push(e.from.clone());
            dependents.entry(e.from.clone()).or_default().push(e.to.clone());
        }

        let mut tasks: HashMap<String, DagTask> = HashMap::new();
        let mut remaining_deps: HashMap<String, usize> = HashMap::new();
        let mut ready_q: VecDeque<String> = VecDeque::new();

        for n in &spec.nodes {
            let d = deps.get(&n.task_id).cloned().unwrap_or_default();
            let rem = d.len();

            let status = if rem == 0 {
                ready_q.push_back(n.task_id.clone());
                DagTaskStatus::Ready
            } else {
                DagTaskStatus::Pending
            };

            let th = task_hash_hex(&meta.dag_root_hash, n);

            tasks.insert(
                n.task_id.clone(),
                DagTask {
                    run_id: run_id.clone(),
                    dag_id: dag_id.to_string(),
                    task_id: n.task_id.clone(),
                    task_hash: th,
                    work_units: n.work_units,
                    task_type: if n.task_type.trim().is_empty() {
                        "sim".to_string()
                    } else {
                        n.task_type.clone()
                    },
                    deps: d,
                    status,
                    assigned_node: None,
                    assigned_unix: None,
                    lease_expires_unix: None,
                    completed_unix: None,
                },
            );

            remaining_deps.insert(n.task_id.clone(), rem);
        }

        let run = DagRunState {
            run_id: run_id.clone(),
            dag_id: dag_id.to_string(),
            dag_root_hash: meta.dag_root_hash,
            name: meta.name,
            created_unix: now_unix(),
            tasks_total: tasks.len(),
            tasks,
            dependents,
            remaining_deps,
            ready_q,
        };

        s.dag_runs.insert(run_id.clone(), run);
        Ok(run_id)
    }

    pub async fn get_dag_run_view(&self, run_id: &str) -> Option<ComputeDagRunView> {
        let s = self.inner.read().await;
        let r = s.dag_runs.get(run_id)?;

        let mut ready = 0usize;
        let mut running = 0usize;
        let mut completed = 0usize;
        let mut failed = 0usize;

        for t in r.tasks.values() {
            match t.status {
                DagTaskStatus::Ready => ready += 1,
                DagTaskStatus::Running => running += 1,
                DagTaskStatus::Completed => completed += 1,
                DagTaskStatus::Failed => failed += 1,
                _ => {}
            }
        }

        Some(ComputeDagRunView {
            run_id: r.run_id.clone(),
            dag_id: r.dag_id.clone(),
            dag_root_hash: r.dag_root_hash.clone(),
            name: r.name.clone(),
            created_unix: r.created_unix,
            tasks_total: r.tasks_total,
            tasks_ready: ready,
            tasks_running: running,
            tasks_completed: completed,
            tasks_failed: failed,
        })
    }

    // =========================================================================
    // ComputeDAG: Scheduling + Completion
    // =========================================================================

    fn expire_dag_task_leases_locked(run: &mut DagRunState, now: u64) {
        // If a running task lease expired -> back to READY (simple requeue)
        let mut requeue: Vec<String> = Vec::new();

        for (task_id, t) in run.tasks.iter_mut() {
            if t.status == DagTaskStatus::Running {
                if let Some(exp) = t.lease_expires_unix {
                    if now >= exp {
                        t.status = DagTaskStatus::Ready;
                        t.assigned_node = None;
                        t.assigned_unix = None;
                        t.lease_expires_unix = None;
                        requeue.push(task_id.clone());
                    }
                }
            }
        }

        for id in requeue {
            run.ready_q.push_back(id);
        }
    }

    pub async fn assign_next_dag_task(
        &self,
        run_id: &str,
        node_id: &str,
    ) -> Option<DagTaskLease> {
        let now = now_unix();
        let mut s = self.inner.write().await;

        if !s.nodes.contains_key(node_id) {
            return None;
        }

        let run = s.dag_runs.get_mut(run_id)?;
        Self::expire_dag_task_leases_locked(run, now);

        while let Some(task_id) = run.ready_q.pop_front() {
            let Some(t) = run.tasks.get_mut(&task_id) else {
                continue;
            };
            if t.status != DagTaskStatus::Ready {
                continue;
            }

            t.status = DagTaskStatus::Running;
            t.assigned_node = Some(node_id.to_string());
            t.assigned_unix = Some(now);
            t.lease_expires_unix = Some(now + DAG_TASK_LEASE_SEC);

            return Some(DagTaskLease {
                run_id: run.run_id.clone(),
                dag_id: run.dag_id.clone(),
                task_id: t.task_id.clone(),
                task_hash: t.task_hash.clone(),
                work_units: t.work_units,
                task_type: t.task_type.clone(),
                lease_expires_unix: t.lease_expires_unix.unwrap_or(now + DAG_TASK_LEASE_SEC),
            });
        }

        None
    }

    pub async fn complete_dag_task(
        &self,
        run_id: &str,
        task_id: &str,
        proof: DagTaskProofSubmitRequest,
    ) -> Result<DagTaskProofSubmitResponse, &'static str> {
        let now = now_unix();
        let mut s = self.inner.write().await;

        // Optional aber mainnet-sinnvoll: node muss existieren
        if !s.nodes.contains_key(&proof.node_id) {
            return Err("node_not_found");
        }

        let run = s.dag_runs.get_mut(run_id).ok_or("run_not_found")?;
        Self::expire_dag_task_leases_locked(run, now);

        let t = run.tasks.get_mut(task_id).ok_or("task_not_found")?;

        if t.status != DagTaskStatus::Running {
            return Err("task_not_running");
        }
        if t.assigned_node.as_deref() != Some(&proof.node_id) {
            return Err("task_wrong_node");
        }
        if proof.work_units != t.work_units {
            return Err("task_wrong_work_units");
        }

        // Mark completed (clean up ownership fields)
        t.status = DagTaskStatus::Completed;
        t.completed_unix = Some(now);
        t.assigned_node = None;
        t.assigned_unix = None;
        t.lease_expires_unix = None;

        // Unlock dependents
        let mut unlocked = 0usize;
        if let Some(children) = run.dependents.get(task_id).cloned() {
            for c in children {
                let rem = run.remaining_deps.get_mut(&c).ok_or("dag_state_corrupt")?;
                if *rem > 0 {
                    *rem -= 1;
                }
                if *rem == 0 {
                    if let Some(ct) = run.tasks.get_mut(&c) {
                        if ct.status == DagTaskStatus::Pending {
                            ct.status = DagTaskStatus::Ready;
                            run.ready_q.push_back(c.clone());
                            unlocked += 1;
                        }
                    }
                }
            }
        }

        Ok(DagTaskProofSubmitResponse {
            status: "accepted".to_string(),
            run_id: run_id.to_string(),
            task_id: task_id.to_string(),
            unlocked_ready: unlocked,
        })
    }

    // =========================================================================
    // Mining Engine
    // =========================================================================

    fn kct_to_nano(kct: f64) -> u64 {
        (kct * (NANO as f64)) as u64
    }

    pub async fn mine_new_block(&self) {
        let now = now_unix();
        let mut s = self.inner.write().await;

        Self::expire_leases_locked(&mut s, now);

        s.block_height += 1;

        let months = months_since(s.genesis_time, now);
        let reward_kct = START_REWARD_KCT * MONTHLY_DECAY.powf(months as f64);
        let reward_nano = Self::kct_to_nano(reward_kct);

        let window = REWARD_WINDOW_SEC;
        let mut weight_map: HashMap<String, f64> = HashMap::new();
        let mut total_wu: f64 = 0.0;

        for p in s.proofs.iter() {
            if now >= p.timestamp_unix && now - p.timestamp_unix <= window {
                // uptime gate
                let allow = s
                    .node_stats
                    .get(&p.node_id)
                    .map(|st| now.saturating_sub(st.first_seen_unix) >= MIN_UPTIME_FOR_REWARDS_SEC)
                    .unwrap_or(false);

                if !allow {
                    continue;
                }

                *weight_map.entry(p.node_id.clone()).or_insert(0.0) +=
                    p.effective_work_units as f64;
                total_wu += p.effective_work_units as f64;
            }
        }

        s.node_last_block_reward.clear();

        if total_wu > 0.0 {
            for (node, wu) in weight_map {
                let share = wu / total_wu;
                let amount = (reward_nano as f64 * share) as u64;
                *s.node_rewards.entry(node.clone()).or_insert(0) += amount;
                s.node_last_block_reward.insert(node, amount);
            }
        }

        s.total_emitted_nano += reward_nano;
    }

    pub async fn mining_stats(&self) -> MiningStats {
        let now = now_unix();
        let s = self.inner.read().await;

        let months = months_since(s.genesis_time, now);
        let reward_kct = START_REWARD_KCT * MONTHLY_DECAY.powf(months as f64);
        let reward_nano = Self::kct_to_nano(reward_kct);

        let window = REWARD_WINDOW_SEC;
        let mut weight_map: HashMap<String, f64> = HashMap::new();
        let mut total_wu: f64 = 0.0;

        for p in s.proofs.iter() {
            if now >= p.timestamp_unix && now - p.timestamp_unix <= window {
                let allow = s
                    .node_stats
                    .get(&p.node_id)
                    .map(|st| now.saturating_sub(st.first_seen_unix) >= MIN_UPTIME_FOR_REWARDS_SEC)
                    .unwrap_or(false);

                if !allow {
                    continue;
                }

                *weight_map.entry(p.node_id.clone()).or_insert(0.0) +=
                    p.effective_work_units as f64;
                total_wu += p.effective_work_units as f64;
            }
        }

        let mut per_node = Vec::new();
        for (node_id, _) in s.nodes.iter() {
            let total = *s.node_rewards.get(node_id).unwrap_or(&0);
            let last = *s.node_last_block_reward.get(node_id).unwrap_or(&0);
            let wu_for_node = *weight_map.get(node_id).unwrap_or(&0.0);
            let share_pct = if total_wu > 0.0 {
                (wu_for_node / total_wu) * 100.0
            } else {
                0.0
            };
            let cumulative_work = *s.node_cumulative_work.get(node_id).unwrap_or(&0);

            per_node.push(NodeMiningStats {
                node_id: node_id.clone(),
                total_mined_nano: total,
                last_block_reward_nano: last,
                hashrate_share_pct: share_pct,
                cumulative_work_units: cumulative_work,
            });
        }

        MiningStats {
            block_height: s.block_height,
            current_block_reward_kct: (reward_nano as f64) / (NANO as f64),
            current_block_reward_nano: reward_nano,
            month_index: months,
            total_emitted_nano: s.total_emitted_nano,
            per_node,
            timestamp: now,
            reward_window_sec: REWARD_WINDOW_SEC,
        }
    }

    // =========================================================================
    // Metrics
    // =========================================================================

    pub async fn compute_metrics(&self, window_sec: u64) -> Metrics {
        let now = now_unix();
        let s = self.inner.read().await;

        let active_nodes_90s = s
            .nodes
            .values()
            .filter(|n| now.saturating_sub(n.last_seen_unix) <= ACTIVE_WINDOW_SEC)
            .count();

        let proofs_window: Vec<&ProofRecord> = s
            .proofs
            .iter()
            .filter(|p| now >= p.timestamp_unix && now - p.timestamp_unix <= window_sec)
            .collect();

        let proofs_count = proofs_window.len();

        let mut active_miners = HashSet::<String>::new();
        for p in proofs_window.iter() {
            if now.saturating_sub(p.timestamp_unix) <= ACTIVE_WINDOW_SEC {
                active_miners.insert(p.node_id.clone());
            }
        }

        let mut completed_jobs = 0usize;
        let mut total_job_ms: u128 = 0;

        for j in s.jobs.values() {
            if let (Some(assigned), Some(done)) = (j.assigned_unix, j.completed_unix) {
                if now >= done && now - done <= window_sec {
                    completed_jobs += 1;
                    total_job_ms += ((done - assigned) as u128) * 1000;
                }
            }
        }

        let avg_job_ms = if completed_jobs > 0 {
            (total_job_ms / completed_jobs as u128) as u64
        } else {
            0
        };

        let jobs_per_min = if window_sec > 0 {
            (completed_jobs as f64) * 60.0 / (window_sec as f64)
        } else {
            0.0
        };

        Metrics {
            window_sec,
            active_nodes_90s,
            active_miners_90s: active_miners.len(),
            jobs_completed_window: completed_jobs,
            jobs_per_min,
            avg_job_ms,
            proofs_window: proofs_count,
            timestamp: now,
        }
    }
}
