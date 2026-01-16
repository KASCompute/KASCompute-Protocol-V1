⚡ KASCompute — Protocol V1



Protocol V1 defines the first-generation off-chain compute protocol used by KASCompute.



It is responsible for coordinating compute activity outside of Kaspa L1, while being explicitly designed to align with Kaspa’s future vProgs execution and settlement layer.



🧩 Scope \& Responsibilities



Protocol V1 currently handles:



🟢 Node presence \& heartbeats



⚙️ Job scheduling



🔐 Cryptographic Proof-of-Compute submission



📊 Metrics \& performance aggregation



Important:

Protocol V1 operates fully off-chain today,

but its data structures and cryptographic design are vProgs-compatible by design.



🎯 Goals of Protocol V1



Deterministic Proof-of-Compute



Simple, inspectable cryptography



Real-time observability



Minimal assumptions about trust



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

&nbsp; "node\_id": "...",

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



offline verification



deterministic replay



future on-chain anchoring



trust-minimized validation



🔌 API Endpoints (V1)

❤️ Heartbeat

POST /v1/nodes/heartbeat





Used for:



node presence



geo enrichment



uptime tracking



role signaling (node / miner)



🧮 Job Scheduling

POST /v1/jobs/next





Returns:



job ID



work units



📤 Proof Submission

POST /v1/jobs/proof





Accepts:



compute result



cryptographic proof



performance metadata



Note:

Extra fields are ignored → forward compatibility guaranteed.



🧠 Verification (Future)



Planned extensions:



Stateless proof verification



Optional replay protection



vProgs-compatible anchoring format



ZK verification layer (post-vProgs)



🧪 Current Status



Protocol: Active



Cryptography: Implemented (SHA-256 + Ed25519)



Settlement: Off-chain



ZK Proofs: Not yet



📌 Notes



Protocol V1 is intentionally minimal.



Complexity is deferred to:



verification layers



settlement logic



future vProgs execution



This keeps the protocol:



auditable



flexible



future-proof



Founder: Tarik Kaya

Built with ⚡ \& 💚 on Kaspa.

