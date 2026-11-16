# Deployment contract

The Kubernetes manifests live outside this repository. This page lists what the
container expects, so the manifests can be written and checked against it.

## Image

- `docker.io/<dockerhub user>/tessera:<tag>`, built by the `image` job in `.github/workflows/ci.yml` for
  `linux/amd64` and `linux/arm64`, published only after all tests pass. Tags: `sha-<full sha>`, `main`, and `X.Y.Z` / `X.Y` from `vX.Y.Z` git tags.
- Base: `gcr.io/distroless/static-debian12:nonroot`. A single static binary, `/tessera`, runs as UID/GID **65532**.
- CI pushes with the repo variable `DOCKERHUB_USERNAME` and the secret `DOCKERHUB_TOKEN` (a Docker Hub access token with Read & Write).
- If the Docker Hub repository is private, the namespace needs an `imagePullSecret`.

## Process

| Item | Value |
|---|---|
| Port | `8080/TCP` (HTTP; TLS terminates at Cloudflare) |
| Liveness | `GET /healthz` → `200 ok` (process only, no DB) |
| Readiness | `GET /readyz` → `200 ok`, `503` if SQLite is unreachable |
| Shutdown | Handles `SIGTERM`: stops accepting, drains in-flight requests, closes the DB |
| Logs | JSON lines on stdout; level via `RUST_LOG` (e.g. `info,tessera=debug`) |
| Startup failure | Invalid config exits non-zero with a message naming the variable |

## Required shape

- **Replicas: 1** and **`strategy: Recreate`**. SQLite allows a single writer; a rolling update would run two pods against one file.
- **Volume**: a `ReadWriteOnce` PVC on the network-backed storage class, mounted at **`/data`**.
  Set `securityContext.fsGroup: 65532` so the volume is writable.
  Card photos live in the database (≤ 512 KB each, typically ~100 KB, two per card): **1 Gi** is plenty for two people.
- Suggested pod security: `runAsNonRoot: true`, `readOnlyRootFilesystem: true`, `allowPrivilegeEscalation: false`, drop all capabilities.
  The app writes only to `/data` (SQLite temp storage is kept in memory).
- Resources: about 20 MiB RAM idle. Requests `cpu: 10m, memory: 32Mi`, limit `memory: 128Mi` is plenty for now.

## Environment

| Variable | Required | Default | Notes |
|---|---|---|---|
| `TESSERA_PUBLIC_URL` | **yes** | – | `https://<host>`; origin only, no path. `http` only for localhost |
| `TESSERA_WEBAUTHN_RP_ID` | no | host of `PUBLIC_URL` | Set to the **apex domain** so the subdomain can change without invalidating passkeys |
| `TESSERA_WEBAUTHN_RP_NAME` | no | `Tessera` | |
| `TESSERA_DATABASE_URL` | no | `sqlite:///data/tessera.db` (image) | |
| `TESSERA_LISTEN_ADDR` | no | `0.0.0.0:8080` (image) | |
| `TESSERA_REGION_ISO3166` | no | `IT` | |
| `TESSERA_NEARBY_RADIUS_M` | no | `200` | 1–5000 |
| `TESSERA_STORE_REFRESH_INTERVAL` | no | `7d` | humantime syntax (`12h`, `7d`) |
| `TESSERA_OVERPASS_URL` | no | `https://overpass-api.de/api/interpreter` | Store locations (OpenStreetMap). Any Overpass mirror works |
| `TESSERA_WIKIDATA_URL` | no | `https://www.wikidata.org/w/api.php` | Brand search |
| `TESSERA_COMMONS_URL` | no | `https://commons.wikimedia.org/w/api.php` | Brand logos (thumbnails are downloaded from `upload.wikimedia.org`) |
| `TESSERA_SESSION_TTL` | no | `30d` | How long a sign-in lasts |
| `TESSERA_BOOTSTRAP_TOKEN` | no | random | Secret. Fixed first-admin token: 32 random bytes, base64url (`openssl rand -base64 32 \| tr '+/' '-_' \| tr -d '='`); weak values are rejected. Unset: a random link is logged at startup while no users exist |
| `RUST_LOG` | no | `info` | |

## First start and account recovery

While the database has no users, every start creates a single-use **first-admin link**, valid 24h:

- Without `TESSERA_BOOTSTRAP_TOKEN`, the link is printed in the logs (`no users yet: open this link…`).
  It stops working as soon as the first account exists.
- With `TESSERA_BOOTSTRAP_TOKEN` (from a Secret), nothing secret is logged: open `https://<host>/join#<token>`.

Admin commands run inside the pod and print links to stdout:

```sh
kubectl exec deploy/tessera -c tessera -- /tessera admin users              # ids, households, passkey counts
kubectl exec deploy/tessera -c tessera -- /tessera admin invite             # household invite (48h, single use)
kubectl exec deploy/tessera -c tessera -- /tessera admin recover <user-id>  # new passkey after losing all devices (24h)
```

Invite secrets live in the URL fragment (`#…`), which browsers never send to the server, so they don't appear in Cloudflare or app logs.

Signing out of compromised devices: completing a recovery link ends all of that user's sessions; removing a passkey ends every session except the current one; Settings → "Sign out of all devices" ends all of them.

## Backups: Litestream sidecar → Cloudflare R2

The database runs in WAL mode, which Litestream needs. Run Litestream as a sidecar sharing the `/data` volume:

```yaml
# litestream.yml (ConfigMap)
dbs:
  - path: /data/tessera.db
    replicas:
      - type: s3
        bucket: <bucket>
        path: tessera
        endpoint: https://<account-id>.r2.cloudflarestorage.com
        region: auto
        force-path-style: true
```

Credentials come from a Secret as `LITESTREAM_ACCESS_KEY_ID` / `LITESTREAM_SECRET_ACCESS_KEY` (an R2 API token scoped to the bucket).
For restores, add an init container: `litestream restore -if-db-not-exists -if-replica-exists /data/tessera.db`.
Then a lost PVC recovers on the next pod start.

### Restore drill (every few months, and after changing the Litestream config)

A backup is only good if it restores. From any machine with the `litestream` and `sqlite3` binaries and the R2 credentials exported:

```sh
# 1. Restore the latest replica to a scratch file (never over the live database).
litestream restore -o /tmp/tessera-check.db \
  's3://<bucket>/tessera?endpoint=https://<account-id>.r2.cloudflarestorage.com&region=auto&forcePathStyle=true'

# 2. The file must be a healthy SQLite database.
sqlite3 /tmp/tessera-check.db 'PRAGMA integrity_check;'   # → ok

# 3. Compare with the live app (the app and Litestream images have no sqlite3).
sqlite3 /tmp/tessera-check.db \
  "SELECT u.display_name, count(*) FROM cards c JOIN users u ON u.id = c.owner_id
   WHERE c.deleted_at IS NULL GROUP BY u.display_name;"

# 4. Clean up: the file holds every card number and photo.
rm /tmp/tessera-check.db*
```

Each user's count should match the number of their own cards in the app (the replica lags by at most a few seconds). If step 1 fails, check the Litestream sidecar logs
(`kubectl logs deploy/tessera -c litestream`) before anything else: a sidecar that stopped replicating does not stop the app.
For a full rehearsal, scale the deployment to 0, delete the PVC in a test namespace, and let the init container restore it.

Users can also keep their own copy: Settings → "Export cards" downloads a JSON file with every card they own (photos included).
That file is **not encrypted**; it is meant for moving cards, not as the server backup.

## Network & Cloudflare

- No Ingress or NodePort. Add a public-hostname route on the existing tunnel → `http://<service>.<namespace>.svc.cluster.local:8080`.
- NetworkPolicy: allow ingress to port 8080 only from the `cloudflared` pods. The app trusts `CF-Connecting-IP` as the client IP for rate limiting
  (20 auth requests/min and 600 API requests/min per IPv4 address or IPv6 /64), which is only safe while nothing else can reach the pod.
  Also allow egress to DNS, R2 (Litestream), and HTTPS (443) to `overpass-api.de` and `www.wikidata.org`
  (brand search and weekly store-location refresh; the app works without them, only "nearby" by brand does not).
  Requests use a generic `User-Agent` (`tessera/<version> (self-hosted loyalty card app)`); users' locations are
  never sent anywhere: "nearby" is computed on the phone.
- WAF custom rule: block when `ip.geoip.country ne "IT"`.
- Caching: the app sends `Cache-Control` on every response: `immutable` for `/assets/*`, `no-cache` for `index.html`/`sw.js`/manifest/icons, `private, immutable` for card photos (`/api/images/*`, browser cache only), `no-store` for everything else (API, probes, errors).
  Keep Cloudflare on "respect origin headers"; don't add "cache everything" rules.
- Don't enable Rocket Loader or HTML minification: they inject scripts that the CSP blocks.
