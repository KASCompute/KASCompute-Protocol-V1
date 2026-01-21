use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagSpec {
    pub name: String,
    pub nodes: Vec<DagNodeSpec>,
    pub edges: Vec<DagEdgeSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagNodeSpec {
    pub task_id: String, // stable id inside dag
    pub work_units: u64,
    #[serde(default)]
    pub task_type: String, // e.g. "sim", "render", "infer"
    #[serde(default)]
    pub params: serde_json::Value, // arbitrary JSON params (future-proof)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagEdgeSpec {
    pub from: String,
    pub to: String,
}

/// Deterministic DAG id: sha256(canonical_json(spec))
pub fn dag_id_hex(spec: &DagSpec) -> String {
    let canonical = canonicalize_spec(spec);
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

/// Deterministic DAG root hash (same as dag_id for now, but separated on purpose)
pub fn dag_root_hash_hex(spec: &DagSpec) -> String {
    dag_id_hex(spec)
}

/// Deterministic task hash binds task to DAG root + task definition.
pub fn task_hash_hex(dag_root_hash_hex: &str, node: &DagNodeSpec) -> String {
    let mut h = Sha256::new();
    h.update(b"task|");
    h.update(dag_root_hash_hex.as_bytes());
    h.update(b"|");
    h.update(node.task_id.as_bytes());
    h.update(b"|");
    h.update(node.work_units.to_le_bytes());
    h.update(b"|");
    h.update(node.task_type.as_bytes());

    let p = canonicalize_json(&node.params);
    let pb = serde_json::to_vec(&p).unwrap_or_default();
    h.update(b"|");
    h.update(pb);

    hex::encode(h.finalize())
}

/// Validate DAG: no duplicate task_id, edges refer to nodes, no self-edge, no cycles.
pub fn validate_dag(spec: &DagSpec) -> Result<(), &'static str> {
    let mut ids = BTreeSet::new();
    for n in &spec.nodes {
        if n.task_id.trim().is_empty() {
            return Err("dag_invalid_task_id");
        }
        if !ids.insert(n.task_id.clone()) {
            return Err("dag_duplicate_task_id");
        }
    }

    for e in &spec.edges {
        if e.from == e.to {
            return Err("dag_self_edge");
        }
        if !ids.contains(&e.from) || !ids.contains(&e.to) {
            return Err("dag_edge_unknown_node");
        }
    }

    // cycle check via Kahn
    let (indeg, out) = build_graph(spec);
    let mut indeg2 = indeg.clone();
    let mut q: Vec<String> = indeg2
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(k, _)| k.clone())
        .collect();

    let mut visited = 0usize;
    while let Some(x) = q.pop() {
        visited += 1;
        if let Some(children) = out.get(&x) {
            for c in children {
                if let Some(d) = indeg2.get_mut(c) {
                    *d = d.saturating_sub(1);
                    if *d == 0 {
                        q.push(c.clone());
                    }
                }
            }
        }
    }

    if visited != spec.nodes.len() {
        return Err("dag_cycle_detected");
    }

    Ok(())
}

fn build_graph(spec: &DagSpec) -> (BTreeMap<String, u64>, BTreeMap<String, Vec<String>>) {
    let mut indeg: BTreeMap<String, u64> = BTreeMap::new();
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in &spec.nodes {
        indeg.insert(n.task_id.clone(), 0);
        out.entry(n.task_id.clone()).or_default();
    }
    for e in &spec.edges {
        out.entry(e.from.clone()).or_default().push(e.to.clone());
        *indeg.entry(e.to.clone()).or_insert(0) += 1;
    }
    (indeg, out)
}

/// Canonicalize ordering: sort nodes by task_id; sort edges by (from,to); canonicalize params JSON.
fn canonicalize_spec(spec: &DagSpec) -> DagSpec {
    let mut nodes = spec.nodes.clone();
    for n in nodes.iter_mut() {
        n.params = canonicalize_json(&n.params);
        if n.task_type.trim().is_empty() {
            n.task_type = "sim".to_string();
        }
    }
    nodes.sort_by(|a, b| a.task_id.cmp(&b.task_id));

    let mut edges = spec.edges.clone();
    edges.sort_by(|a, b| (a.from.clone(), a.to.clone()).cmp(&(b.from.clone(), b.to.clone())));

    DagSpec {
        name: spec.name.clone(),
        nodes,
        edges,
    }
}

/// Deterministic JSON: sort object keys recursively
fn canonicalize_json(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                out.insert(k.clone(), canonicalize_json(&map[&k]));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(arr) => serde_json::Value::Array(arr.iter().map(canonicalize_json).collect()),
        _ => v.clone(),
    }
}
