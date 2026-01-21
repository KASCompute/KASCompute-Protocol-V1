⚡ KASCompute — Protocol V1



Protocol V1 defines the first-generation off-chain compute protocol used by KASCompute.



It coordinates decentralized compute execution outside of Kaspa L1, while being explicitly designed to align with Kaspa’s future vProgs execution and settlement layer.



Protocol V1 is minimal by design, deterministic, and built to evolve toward on-chain settlement without refactoring the core protocol.



🧩 Scope \& Responsibilities



Protocol V1 currently handles:



🟢 Node presence \& heartbeats



⚙️ Job scheduling



🔐 Cryptographic Proof-of-Compute submission



📊 Metrics \& performance aggregation



🧠 ComputeDAG execution (task graphs with dependencies)



Important



Protocol V1 operates fully off-chain today,

but its data structures, hashing model, and cryptographic flows are vProgs-compatible by design.



🎯 Goals of Protocol V1



Deterministic Proof-of-Compute



Simple, inspectable cryptography



Real-time observability



Minimal trust assumptions



DAG-based execution for complex workloads



ZK-ready structure for future settlement



🔐 Proof-of-Compute (V1)



Each compute proof follows a strict, deterministic pipeline.



Proof construction steps



Create a deterministic JSON payload



Serialize payload to bytes



Compute SHA-256(payload\_bytes)



Sign the hash using Ed25519



Attach public key for verification



📦 Proof Payload (example)

{

&nbsp; "node\_id": "kc\_node\_01",

&nbsp; "job\_id": 123,

&nbsp; "work\_units": 42,

&nbsp; "workload\_mode": "sim",

&nbsp; "elapsed\_ms": 150,

&nbsp; "client\_version": "launcher-miner/0.2.0",

&nbsp; "ts": 1710000000

}



✍️ Cryptographic Fields (sent with proof)

{

&nbsp; "proof\_hash": "hex",

&nbsp; "signature": "hex",

&nbsp; "public\_key\_hex": "hex"

}





This enables:



Offline verification



Deterministic replay



Future on-chain anchoring



Trust-minimized validation



🧠 ComputeDAG Execution (Protocol V1)



Protocol V1 includes a native DAG-based execution engine for structured compute workloads.



A ComputeDAG consists of:



Tasks (nodes)



Explicit dependencies (edges)



Deterministic task hashing



Run-scoped execution state



Key properties



Deterministic DAG root hash



Dependency-aware scheduling



FIFO-ready queue per run



Stateless workers (nodes do not coordinate with each other)



🔁 Task Leasing Model



Each task is leased to a node for a fixed time window.



Tasks transition: Pending → Ready → Running → Completed



A running task is owned by exactly one node



Leases expire automatically



Leases can be renewed explicitly by the owning node



This prevents:



Stalled execution



Double execution



Long-running task lockups



🔄 Lease Renewal



Nodes may extend an active lease:



POST /v1/runs/:run\_id/tasks/:task\_id/renew





Rules:



Only the assigned node may renew



Expired leases cannot be renewed



Renewal preserves deterministic execution



✅ Task Completion



Submitting a task proof:



POST /v1/runs/:run\_id/tasks/:task\_id/proof





On completion:



Task is marked Completed



Lease is cleared



Dependent tasks are unlocked deterministically



Newly ready tasks are queued



🔌 API Endpoints (V1)

❤️ Heartbeat

POST /v1/nodes/heartbeat





Used for:



Node presence



Geo enrichment



Uptime tracking



Role signaling (node, miner)



🧮 Job Scheduling

POST /v1/jobs/next





Returns:



Job ID



Work units



Lease expiration



📤 Proof Submission

POST /v1/jobs/proof





Accepts:



Compute result



Cryptographic proof



Performance metadata



Note

Extra fields are ignored → forward compatibility guaranteed.



🧠 ComputeDAG (V1)

POST /v1/dags/submit

POST /v1/dags/:dag\_id/runs

POST /v1/runs/:run\_id/next

POST /v1/runs/:run\_id/tasks/:task\_id/proof

POST /v1/runs/:run\_id/tasks/:task\_id/renew

GET  /v1/runs/:run\_id



🧪 Current Status



Protocol: Active



ComputeDAG: Active



Cryptography: Implemented (SHA-256 + Ed25519)



Leasing \& Renewals: Implemented



Settlement: Off-chain



ZK Proofs: Not yet



🧠 Verification (Future)



Planned extensions:



Stateless proof verification



Optional replay protection



vProgs-compatible anchoring format



ZK verification layer (post-vProgs)



📌 Design Notes



Protocol V1 is intentionally minimal.



Complexity is deferred to:



Verification layers



Settlement logic



Future vProgs execution



This keeps the protocol:



Auditable



Flexible



Future-proof



Founder: Tarik Kaya

Built with ⚡ \& 💚 on Kaspa

