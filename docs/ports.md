# EST port registry

Loopback (`127.0.0.1`) and tailnet ports for EST services and
dashboards. No product binds a port without registering it here
first — a PR editing this file. Nothing registered here is ever
reachable off the tailnet.

Block: `18920–18929` (10 ports; request a new block before
improvising outside it).

| Port | Product | Notes |
|---|---|---|
| 18923 | angel (legacy) | Stays until the Angel sunset; then retired, never reused |
| 18924 | est-vault | Reserved for the future Vault panel |
| 18925 | est-hub | VPS API + queue; binds the tailnet IP only, never public |

All other ports in the block are unassigned.
