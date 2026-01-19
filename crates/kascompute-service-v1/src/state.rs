use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;

use rand::Rng;
use tokio::sync::RwLock;

use crate::domain::models::*;
use crate::util::geo::geo_lookup;
use crate::util::receipt::{make_receipt, verify_ed25519_signature_hex, verify_proof_signature_v1, ProofPayloadV1};

use crate::util::time::{months_since, now_unix};

// =========================
// Protocol constants (demo/testnet)
// =========================

pub const NANO: u64 = 100_000_000; // 1 KCT = 100_000_000 nanoKCT
pub const START_REWARD_KCT: f64 = 200.0;
pub const MONTHLY_DECAY: f64 = 0.99; // -1% per month
pub const VERIFIED_BONUS_MULT: f64 = 1.10;

pub const MIN_UPTIME_FOR_REWARDS_SEC: u64 = 60;
pub const REWARD_WINDOW_SEC: u64 = 300; // same as your 5-min weight window
pub const ACTIVE_WINDOW_SEC: u64 = 90;

pub const JOB_SCHEDULE_EVERY_SEC: u64 = 10;
pub const JOB_LEASE_SEC: u64 = 60;

// =========================

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<RwLock<InnerState>>,
}

pub struct InnerState {
    pub nodes: HashMap<String, Node>,
    pub jobs: HashMap<u64, Job>,
    pub proofs: Vec<ProofRecord>,

    pub next_job_id: u64,
    pub genesis_time: u64,
    pub block_height: u64,
    pub total_emitted_nano: u64,

    pub node_rewards: HashMap<String, u64>,
    pub node_last_block_reward: HashMap<String, u64>,
    pub node_cumulative_work: HashMap<String, u64>,
    pub node_stats: HashMap<String, NodeStats>,

    pub demo_running: bool,
}

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
        };

        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    // -------------------------
    // Read helpers
    // -------------------------
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

    // -------------------------
    // Heartbeats
    // -------------------------
    pub async fn upsert_node(&self, mut payload: HeartbeatPayload, client_ip: IpAddr) {
        let now = now_unix();

        // Fill geo if missing
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

        // ensure accounting maps exist
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

    // -------------------------
    // Jobs / scheduler
    // -------------------------
    pub async fn create_scheduled_job(&self, is_demo: bool) -> u64 {
        // IMPORTANT: rng must not live across .await (Send issue)
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
        let now = now_unix();
        let mut s = self.inner.write().await;

        Self::expire_leases_locked(&mut s, now);

        if !s.nodes.contains_key(node_id) {
            return None;
        }

        // pick first pending
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

    // -------------------------
    // Proof submit + signature verification (optional)
    // -------------------------
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

    // workload mode is informational only (NOT security-critical)
    let mode = proof
        .workload_mode
        .clone()
        .unwrap_or_else(|| "sim".to_string());

    // miner time (fallback)
    let elapsed_ms = proof.elapsed_ms.unwrap_or_else(|| {
        if let Some(a) = job.assigned_unix {
            (ts.saturating_sub(a)) * 1000
        } else {
            0
        }
    });

// -------------------------
// signature verification
// V1 (launcher): ed25519( sha256( json(payload) ) )
// fallback: legacy "pf|..."
// -------------------------
let mut sig_ok = false;

// timestamp: prefer miner provided timestamp_unix, otherwise server time
let ts_msg = proof.timestamp_unix.unwrap_or(ts);

// optional replay protection ±60s (only if miner sent ts)
if let Some(ts_sent) = proof.timestamp_unix {
    if ts.abs_diff(ts_sent) > 60 {
        // too old/new -> invalid (keeps sig_ok false)
    }
}

if let (Some(sig_hex), Some(node)) = (proof.signature_hex.clone(), s.nodes.get(&proof.node_id)) {
    // Try V1 verify first (works with your new launcher)
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

    // Fallback legacy verify (older clients)
    if !sig_ok {
        let legacy_msg = format!(
            "pf|{}|{}|{}|{}",
            proof.node_id, job_id, proof.work_units, ts_msg
        );
        sig_ok = verify_ed25519_signature_hex(&node.public_key_hex, &sig_hex, &legacy_msg);
    }
}

// STRICT verify (optional): only enforce if client explicitly declares protocol-v1
// NOTE: your launcher sends "launcher-miner/0.2.0" so DO NOT enforce here or you'd reject everything.
if proof.client_version.as_deref() == Some("protocol-v1") && !sig_ok {
    return Err("invalid_signature");
}


    // -------------------------
    // effective work: bonus ONLY if signature verified
    // -------------------------
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

    // node stats (effective + verified raw)
    let st = s.node_stats.entry(proof.node_id.clone()).or_insert(NodeStats {
        node_id: proof.node_id.clone(),
        first_seen_unix: ts,
        last_seen_unix: ts,
        total_effective_work_units: 0,
        verified_work_units: 0,
    });

    st.last_seen_unix = ts;
    st.total_effective_work_units += effective_wu;

    // verified_work_units reflects signature verification
    if sig_ok {
        st.verified_work_units += proof.work_units;
    }

    Ok(record)
}


    // -------------------------
    // Mining engine
    // -------------------------
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

    // -------------------------
    // Metrics
    // -------------------------
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

    // -------------------------
    // Demo mode
    // -------------------------
    pub async fn demo_spawn(&self, nodes: usize, jobs: usize) {
        let now = now_unix();
        let mut s = self.inner.write().await;
        s.demo_running = true;

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

        // IMPORTANT: rng must not live across .await
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
}
