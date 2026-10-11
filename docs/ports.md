# EST port registry

Loopback (`127.0.0.1`) and tailnet ports for EST services and
dashboards. No product binds a port without registering it here first —
a PR editing this file.

Block: `18920–18929` (10 ports; request a new block before
improvising outside it).

| Port | Product | Notes |
|---|---|---|
| 18923 | retired (was angel/legacy) | Sunset done; never reused |
| 18924 | est-vault | Reserved for the future Vault panel |
| 18925 | est-hub | VPS API + queue; binds the tailnet IP only, never public |

Nothing here is ever reachable off the tailnet. The hub has no public
listener: the phone reaches it over the tailnet, and the phone's HTTPS
endpoint (`https://<hub>.<tailnet>.ts.net:8443`) is Tailscale's, not a hub
port.

All other ports in the block are unassigned.
