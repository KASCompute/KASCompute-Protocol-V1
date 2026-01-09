use anyhow::{anyhow, Context, Result};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    convert::TryInto,
    fs,
    path::Path,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Serialize, Deserialize)]
struct NodeConfig {
    /// Unique identifier for this node
    node_id: String,
    /// "cpu" | "gpu-amd" | "gpu-nvidia" | "mixed"
    compute_profile: String,
    /// "sim" | "hash" (hash = verified mode)
    workload_mode: String,
    /// API base, optional (overridden by env KASCOMPUTE_API)
    api_base: Option<String>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: "kc_node_01".to_string(),
            compute_profile: "cpu".to_string(),
            workload_mode: "sim".to_string(),
            api_base: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct NodeIdentity {
    public_key_hex: String,
    secret_key_hex: String,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn api_base_url(cfg: &NodeConfig) -> String {
    std::env::var("KASCOMPUTE_API")
        .ok()
        .or_else(|| cfg.api_base.clone())
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn ensure_config(path: &str) -> Result<NodeConfig> {
    let cfg_path = Path::new(path);
    if !cfg_path.exists() {
        let default_cfg = NodeConfig::default();
        if let Some(parent) = cfg_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Could not create config directory {:?}", parent))?;
        }
        let yaml = serde_yaml::to_string(&default_cfg)?;
        fs::write(cfg_path, yaml).with_context(|| format!("Could not write default config to {}", path))?;
        println!("➜ Created default node config at: {}", path);
        println!("  Edit this file and restart the node.\n");
    }
    let contents = fs::read_to_string(cfg_path).with_context(|| format!("Could not read node config from {}", path))?;
    let cfg: NodeConfig = serde_yaml::from_str(&contents).with_context(|| format!("Invalid YAML in {}", path))?;
    Ok(cfg)
}

fn ensure_identity(path: &str) -> Result<(SigningKey, VerifyingKey)> {
    let id_path = Path::new(path);

    if !id_path.exists() {
        let mut rng = OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verify_key = signing_key.verifying_key();

        let identity = NodeIdentity {
            public_key_hex: hex::encode(verify_key.to_bytes()),
            secret_key_hex: hex::encode(signing_key.to_bytes()),
        };

        if let Some(parent) = id_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Could not create identity directory {:?}", parent))?;
        }
        let json = serde_json::to_string_pretty(&identity)?;
        fs::write(id_path, json).with_context(|| format!("Could not write identity file to {}", path))?;

        println!("➜ Generated new node keypair at: {}", path);
        println!("  Public key (hex): {}", identity.public_key_hex);
        println!("  Keep this file secret and back it up.\n");

        return Ok((signing_key, verify_key));
    }

    let data = fs::read_to_string(id_path).with_context(|| format!("Could not read identity file from {}", path))?;
    let identity: NodeIdentity = serde_json::from_str(&data).with_context(|| format!("Invalid JSON in identity file {}", path))?;

    let sk_bytes = hex::decode(identity.secret_key_hex.clone())
        .map_err(|e| anyhow!("Invalid secret key hex: {e}"))?;
    if sk_bytes.len() != 32 {
        return Err(anyhow!("Invalid secret key length (expected 32, got {})", sk_bytes.len()));
    }
    let sk_array: [u8; 32] = sk_bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("Failed to convert secret key to [u8; 32]"))?;

    let signing_key = SigningKey::from_bytes(&sk_array);
    let verify_key = signing_key.verifying_key();
    Ok((signing_key, verify_key))
}

/* =========================
   V1 API DTOs
   ========================= */

#[derive(Debug, Serialize)]
struct HeartbeatRequestV1 {
    node_id: String,
    public_key_hex: String,
    // keep compatible with your server: it will ignore extra fields if not used
    #[serde(default)]
    compute_profile: Option<String>,
    #[serde(default)]
    workload_mode: Option<String>,
    timestamp_unix: u64,
    signature_hex: String,
}

#[derive(Debug, Deserialize)]
struct NextJobResponseV1 {
    status: String,
    data: Option<NextJobData>,
    #[serde(default)]
    error: Option<ApiError>,
    ts: u64,
}

#[derive(Debug, Deserialize)]
struct NextJobData {
    job: Option<JobLease>,
}

#[derive(Debug, Deserialize)]
struct JobLease {
    id: u64,
    work_units: u64,
}

#[derive(Debug, Serialize)]
struct NextJobRequestV1 {
    node_id: String,
}

#[derive(Debug, Serialize)]
struct ProofSubmitV1 {
    node_id: String,
    work_units: u64,
    #[serde(default)]
    workload_mode: Option<String>,
    #[serde(default)]
    elapsed_ms: Option<u64>,
    #[serde(default)]
    result_hash: Option<String>,
    #[serde(default)]
    client_version: Option<String>,
    timestamp_unix: u64,
    signature_hex: String,
}

#[derive(Debug, Deserialize)]
struct ApiOk {
    status: String,
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<ApiError>,
    ts: u64,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: String,
    message: String,
}

/* =========================
   Signatures (canonical)
   =========================
   heartbeat msg: "hb|<node_id>|<compute_profile>|<ts>"
   proof msg:     "pf|<node_id>|<job_id>|<work_units>|<ts>"
*/

fn sign_heartbeat(signing_key: &SigningKey, node_id: &str, compute_profile: &str, ts: u64) -> String {
    let msg = format!("hb|{}|{}|{}", node_id, compute_profile, ts);
    let sig = signing_key.sign(msg.as_bytes());
    hex::encode(sig.to_bytes())
}

fn sign_proof(signing_key: &SigningKey, node_id: &str, job_id: u64, work_units: u64, ts: u64) -> String {
    let msg = format!("pf|{}|{}|{}|{}", node_id, job_id, work_units, ts);
    let sig = signing_key.sign(msg.as_bytes());
    hex::encode(sig.to_bytes())
}

/* =========================
   Workload
   ========================= */

fn cpu_workload(iterations: u64) -> (u64, String) {
    let mut hasher = Sha256::new();
    for i in 0..iterations {
        hasher.update(i.to_le_bytes());
    }
    let result = hasher.finalize();
    (iterations, hex::encode(result))
}

fn amd_gpu_workload(base: u64) -> (u64, String) {
    cpu_workload(base * 8)
}

fn nvidia_gpu_workload(base: u64) -> (u64, String) {
    cpu_workload(base * 6)
}

fn mixed_workload() -> (u64, String) {
    let (cpu_units, cpu_hash) = cpu_workload(50_000);
    let (gpu_units, gpu_hash) = amd_gpu_workload(20_000);
    let mut hasher = Sha256::new();
    hasher.update(cpu_hash.as_bytes());
    hasher.update(gpu_hash.as_bytes());
    let final_hash = hasher.finalize();
    (cpu_units + gpu_units, hex::encode(final_hash))
}

fn run_workload(profile: &str, target_work_units: u64) -> (u64, String, String) {
    // For now we treat target_work_units as a "hint". We keep it simple:
    // - sim mode: quick
    // - hash/verified: use more iterations to feel "heavier"
    match profile {
        "cpu" => {
            let it = std::cmp::max(50_000, target_work_units.min(2_000_000));
            let (u, h) = cpu_workload(it);
            (u, h, "cpu".to_string())
        }
        "gpu-amd" => {
            let it = std::cmp::max(40_000, target_work_units.min(1_000_000));
            let (u, h) = amd_gpu_workload(it);
            (u, h, "gpu-amd".to_string())
        }
        "gpu-nvidia" => {
            let it = std::cmp::max(40_000, target_work_units.min(1_000_000));
            let (u, h) = nvidia_gpu_workload(it);
            (u, h, "gpu-nvidia".to_string())
        }
        "mixed" => {
            let (u, h) = mixed_workload();
            (u, h, "mixed".to_string())
        }
        other => {
            let (u, h) = cpu_workload(80_000);
            (u, h, format!("cpu(fallback:{})", other))
        }
    }
}

/* =========================
   HTTP calls
   ========================= */

fn send_heartbeat_v1(
    client: &Client,
    api_base: &str,
    cfg: &NodeConfig,
    verify_key: &VerifyingKey,
    signing_key: &SigningKey,
) -> Result<()> {
    let url = format!("{}/v1/nodes/heartbeat", api_base);
    let ts = now_unix();

    let signature_hex = sign_heartbeat(signing_key, &cfg.node_id, &cfg.compute_profile, ts);

    let payload = HeartbeatRequestV1 {
        node_id: cfg.node_id.clone(),
        public_key_hex: hex::encode(verify_key.to_bytes()),
        compute_profile: Some(cfg.compute_profile.clone()),
        workload_mode: Some(cfg.workload_mode.clone()),
        timestamp_unix: ts,
        signature_hex,
    };

    let resp = client.post(&url).json(&payload).send()?;
    if !resp.status().is_success() {
        println!("[NODE {}] Heartbeat failed: {}", cfg.node_id, resp.status());
    } else {
        println!("[NODE {}] Heartbeat OK", cfg.node_id);
    }
    Ok(())
}

fn next_job_v1(client: &Client, api_base: &str, node_id: &str) -> Result<Option<JobLease>> {
    let url = format!("{}/v1/jobs/next", api_base.trim_end_matches('/'));
    let payload = NextJobRequestV1 {
        node_id: node_id.to_string(),
    };

    let resp = client.post(&url).json(&payload).send()?;
    let body: NextJobResponseV1 = resp
        .json()
        .context("Failed to decode /v1/jobs/next response")?;

    // Use server timestamp to avoid dead_code warning and aid debugging
    println!("[NODE {}] next_job server ts={}", node_id, body.ts);

    if body.status != "ok" {
        if let Some(e) = body.error {
            println!("[NODE {}] next_job error: {} {}", node_id, e.code, e.message);
        } else {
            println!("[NODE {}] next_job error: unknown error", node_id);
        }
        return Ok(None);
    }

    Ok(body.data.and_then(|d| d.job))
}


fn submit_proof_v1(
    client: &Client,
    api_base: &str,
    cfg: &NodeConfig,
    signing_key: &SigningKey,
    job_id: u64,
    work_units: u64,
    result_hash: String,
    elapsed_ms: u64,
) -> Result<()> {
    let url = format!(
        "{}/v1/jobs/{}/proof",
        api_base.trim_end_matches('/'),
        job_id
    );

    let ts = now_unix();
    let signature_hex = sign_proof(
        signing_key,
        &cfg.node_id,
        job_id,
        work_units,
        ts,
    );

    let payload = ProofSubmitV1 {
        node_id: cfg.node_id.clone(),
        work_units,
        workload_mode: Some(cfg.workload_mode.clone()),
        elapsed_ms: Some(elapsed_ms),
        result_hash: Some(result_hash.clone()),
        client_version: Some("protocol-v1".to_string()),
        timestamp_unix: ts,
        signature_hex,
    };

    let resp = client.post(&url).json(&payload).send()?;
    let body: ApiOk = resp
        .json()
        .context("Failed to decode proof submit response")?;

    if body.status != "ok" {
        if let Some(e) = body.error {
            println!(
                "[NODE {}] submit rejected: {} {}",
                cfg.node_id, e.code, e.message
            );
        } else {
            println!(
                "[NODE {}] submit rejected: unknown error",
                cfg.node_id
            );
        }
        return Ok(());
    }

    println!(
        "[NODE {}] Proof submitted OK for job #{}",
        cfg.node_id, job_id
    );

    if let Some(data) = body.data.as_ref() {
        println!(
            "[NODE {}] Proof response data={}",
            cfg.node_id, data
        );
    }
    println!(
        "[NODE {}] Server ts={}",
        cfg.node_id, body.ts
    );

    Ok(())
}



/* =========================
   Main
   ========================= */

fn main() -> Result<()> {
    println!();
    println!("========================================");
    println!(" KASCompute Node Launcher (Protocol V1)");
    println!(" Heartbeat + Jobs + Proof Submit");
    println!("========================================");
    println!();

    let config_path = "configs/node-config.yaml";
    let identity_path = "configs/node-identity.json";

    let cfg = ensure_config(config_path)?;
    let (signing_key, verify_key) = ensure_identity(identity_path)?;

    let api_base = api_base_url(&cfg);
    println!("Node ID         : {}", cfg.node_id);
    println!("Compute profile : {}", cfg.compute_profile);
    println!("Workload mode   : {}", cfg.workload_mode);
    println!("API base        : {}", api_base);
    println!("Public key      : {}", hex::encode(verify_key.to_bytes()));
    println!();

    let client = Client::new();

    loop {
        // 1) heartbeat
        if let Err(e) = send_heartbeat_v1(&client, &api_base, &cfg, &verify_key, &signing_key) {
            eprintln!("[NODE {}] heartbeat error: {:?}", cfg.node_id, e);
        }

        // 2) ask for job
        match next_job_v1(&client, &api_base, &cfg.node_id) {
            Ok(Some(job)) => {
                println!("[NODE {}] Got job #{} ({} wu)", cfg.node_id, job.id, job.work_units);
		


                // 3) do workload
                let t0 = std::time::Instant::now();
                let (work_units, result_hash, used_profile) = run_workload(&cfg.compute_profile, job.work_units);
                let elapsed_ms = t0.elapsed().as_millis() as u64;

                println!(
                    "[NODE {}] Work done profile={} work_units={} elapsed={}ms",
                    cfg.node_id, used_profile, work_units, elapsed_ms
                );

                // 4) submit proof
                if let Err(e) = submit_proof_v1(
                    &client,
                    &api_base,
                    &cfg,
                    &signing_key,
                    job.id,
                    work_units,
                    result_hash,
                    elapsed_ms,
                ) {
                    eprintln!("[NODE {}] submit proof error: {:?}", cfg.node_id, e);
                }
            }
            Ok(None) => {
                // no job available
            }
            Err(e) => eprintln!("[NODE {}] next_job error: {:?}", cfg.node_id, e),
        }

        thread::sleep(Duration::from_secs(10));
    }
}
