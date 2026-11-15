# Tessera — Implementation Plan

Personal, self-hosted loyalty-card wallet (Stocard / Klarna-cards style).
Rust backend + React/TypeScript PWA, used from the browser on iPhone and Android.
Users: me + my wife, with shared cards.

## 1. Goals & non-goals

**Goals**
- Store loyalty cards (barcode/QR + metadata) and show them fullscreen at the till.
- Works offline (shop basements have no signal).
- Add cards by camera scan, manual entry, or image import.
- Two users, each with private cards, plus cards shared with the household.
- When the app opens, cards for shops near me are pinned at the top.

**Non-goals (for now)**
- App stores, native apps, background geofencing / proximity notifications.
- Apple/Google Wallet pass export (possible later experiment).
- Points balances synced from retailers' APIs.

## 2. Key decisions

| Topic | Decision | Why |
|---|---|---|
| Backend | Rust, `axum`, `sqlx` + SQLite, `tokio` | Single binary, trivial to run on a home server |
| Frontend | React + TypeScript + Vite, `vite-plugin-pwa`, Tailwind CSS | Same as other projects; installable PWA |
| Frontend libs | React Router, TanStack Query, Dexie (IndexedDB), Zod | Routing, server state, offline store, validation at boundaries |
| Barcode render | `bwip-js` | Covers EAN-13/8, UPC-A, Code128, Code39, QR, PDF417, Aztec, DataMatrix |
| Barcode scan | `zxing-wasm` (ZXing-C++ → WASM) | iOS Safari has no `BarcodeDetector`; one code path for both platforms |
| Shared types | `ts-rs` generates TS types from Rust DTOs | No drift between API and client |
| Auth | Passkeys via `webauthn-rs`, cookie sessions (`tower-sessions` + SQLite store) | Face ID / fingerprint on both phones, no passwords |
| Onboarding | Single-use invite links | Wife joins the household without open registration |
| Hosting | Home k8s cluster (arm64 + amd64 nodes), exposed via the **existing** in-cluster Cloudflare Tunnel (new public-hostname route) | Public HTTPS with no open ports or Ingress; camera, geolocation and passkeys need HTTPS |
| Storage | Network-backed storage class (pod may move between nodes) | No node pinning |
| Backups | Litestream → Cloudflare R2 (S3 API, region `auto`) | Cheap, zero egress, already a Cloudflare user |
| CI/CD | GitHub Actions → GHCR (multi-arch image) | Repo lives on GitHub |
| Config | 12-factor env vars (see §5a) | Same image for every environment |
| Container | Multi-arch image (`linux/amd64`, `linux/arm64`), static musl binary on distroless | Pod can land on any node |
| Persistence | Single replica, `Recreate` strategy, RWO PVC; Litestream sidecar replicating SQLite to S3-compatible storage | SQLite has one writer; continuous off-node backup |
| Images | Stored as BLOBs in SQLite (re-encoded to WebP, ≤ ~200 KB) | One file to back up; Litestream covers everything |
| Edge security | Cloudflare WAF rule: allow only my country; app-level rate limiting | Public exposure shrinks attack surface cheaply |
| IDs | UUIDv7 generated client-side | Cards can be created offline and synced later |
| Store locations | OpenStreetMap Overpass API, by `brand:wikidata`, limited to one country (`REGION_ISO3166` config) | Free, good chain coverage, bounded data size |
| Nearby computation | **On the device**, against store locations synced to IndexedDB | Works offline; my location never leaves the phone |
| Asset serving | Frontend embedded in the binary (`rust-embed`) | One artifact to deploy |

## 3. Architecture

```
 Phone (PWA)                     Cloudflare            Home k8s cluster
┌───────────────────────────┐   ┌────────────┐   ┌──────────────────────────────────────┐
│ React UI + Tailwind       │   │ TLS, WAF   │   │ cloudflared ──► Service ──► Pod      │
│  ├─ TanStack Query        │──►│ (country   │──►│   (outbound     (ClusterIP)  ├─ app  │
│  ├─ Dexie (IndexedDB)     │   │  allowlist)│   │    tunnel)                   │  axum │
│  │   cards, images, stores│◄──│            │◄──│                              │  /api │
│  ├─ outbox (offline)      │   └────────────┘   │                              │  PWA  │
│  ├─ zxing-wasm / bwip-js  │                    │                              │  jobs │
│  └─ Geolocation → nearby  │                    │                              └─ lite-│
└───────────────────────────┘                    │                                stream │
                                                 │ PVC: SQLite ──(replicate)──► S3 bucket│
                                                 └──────────────────────────────────────┘
```

No Ingress and no NodePort: the app Service is reachable only from `cloudflared`
(enforced with a NetworkPolicy). That makes it safe to trust the `CF-Connecting-IP` header.

**Offline-first flow**
- The UI always reads from IndexedDB.
- Writes go to IndexedDB and an *outbox*. The outbox replays idempotent `PUT`s when online.
- Pull sync: `GET /api/changes?since=<cursor>` returns changed cards, prefs, tombstones, and store locations for linked brands.
- Conflicts: last-write-wins on a server-assigned `updated_at`. That is fine for 2 users.

## 4. Data model (SQLite)

```
users            (id, display_name, is_admin, created_at)
passkeys         (id, user_id → users, credential_json, name, created_at, last_used_at)
households       (id, name, created_at)
household_members(household_id, user_id, role['owner'|'member'], PK(household_id,user_id))
invites          (token_hash, household_id, created_by, expires_at, used_at)

cards            (id UUID, owner_id → users, household_id NULL → households,  -- set = shared
                  store_name, brand_id NULL → brands, card_number, barcode_format,
                  member_name, color, notes,
                  created_at, updated_at, deleted_at, change_seq)
card_user_prefs  (card_id, user_id, is_favorite, use_count, last_used_at)      -- per-user
card_images      (id, card_id, kind['front'|'back'|'logo'], data BLOB, mime, sha256, created_at)

brands           (id, wikidata_id UNIQUE, name, logo_url, last_synced_at)
store_locations  (id, brand_id, osm_type, osm_id, name, lat, lon, address, updated_at)
                 INDEX (brand_id), UNIQUE (osm_type, osm_id)
custom_places    (id, card_id, label, lat, lon, radius_m)                      -- for shops missing in OSM
```

**Sharing rule:** a user can see a card if `owner_id = me` OR `household_id` is one of my households.
Only the owner can unshare or delete. Household members can edit shared cards.
One check, `can_view` / `can_edit` in a `policy` module, used by every handler and covered by tests.

**Change tracking:** `change_seq` is a global monotonically increasing counter, bumped on every write.
The sync cursor is the last `change_seq` the client has seen.

## 5. API (v1)

| Method & path | Purpose |
|---|---|
| `POST /api/auth/register/{start,finish}` | Register a passkey. Needs an invite token, or the bootstrap token for the first admin |
| `POST /api/auth/login/{start,finish}` | Passkey login (discoverable credentials) |
| `POST /api/auth/logout` · `GET /api/me` | Session |
| `GET/POST/DELETE /api/me/passkeys` | Add a second device, revoke a passkey |
| `GET /api/household` · `POST /api/household/invites` | Members, create an invite link |
| `GET /api/changes?since=` | Pull sync |
| `PUT /api/cards/{id}` · `DELETE /api/cards/{id}` | Idempotent upsert / soft delete |
| `PUT /api/cards/{id}/prefs` | Favourite, usage bump |
| `POST /api/cards/{id}/images` · `GET /api/images/{id}` | Upload (size/MIME validated), authorized download |
| `GET /api/brands/search?q=` | Search local brands, falling back to Wikidata |
| `POST /api/brands` | Link a brand (wikidata id) and trigger a location fetch |

Error envelope: `{ "error": { "code": "...", "message": "..." } }` with proper HTTP status codes.

## 5a. Configuration (env vars)

Validated at startup; the process exits with a clear message if any required value is missing or malformed.
Prefix: `TESSERA_`.

| Variable | Example | Notes |
|---|---|---|
| `TESSERA_PUBLIC_URL` | `https://tessera.example.com` | Exact origin the browser sees; used as WebAuthn origin and for the Origin check |
| `TESSERA_WEBAUTHN_RP_ID` | `example.com` | Defaults to the host of `PUBLIC_URL`. Setting it to the **apex domain** lets the subdomain change later without invalidating passkeys |
| `TESSERA_WEBAUTHN_RP_NAME` | `Tessera` | Shown in the passkey prompt |
| `TESSERA_DATABASE_URL` | `sqlite:///data/tessera.db` | On the PVC |
| `TESSERA_LISTEN_ADDR` | `0.0.0.0:8080` | |
| `TESSERA_REGION_ISO3166` | `IT` | Country for Overpass store fetches |
| `TESSERA_NEARBY_RADIUS_M` | `200` | Default, overridable per user in the UI |
| `TESSERA_STORE_REFRESH_INTERVAL` | `7d` | Overpass refresh period |
| `TESSERA_SESSION_TTL` | `30d` | |
| `TESSERA_BOOTSTRAP_TOKEN` | *(Secret)* | Optional; otherwise a one-time link is logged on first start |
| `RUST_LOG` | `info,tessera=debug` | |

Litestream sidecar (Secret): `LITESTREAM_ACCESS_KEY_ID`, `LITESTREAM_SECRET_ACCESS_KEY`,
plus bucket/endpoint (`https://<account-id>.r2.cloudflarestorage.com`) in its ConfigMap.

**Passkey caveat:** the RP ID can be changed by env var, but passkeys registered under the old RP ID stop working.
Changing only the subdomain is safe if the RP ID is the apex domain.

## 6. Repository layout

```
loyalty/
├─ backend/
│  ├─ src/
│  │  ├─ main.rs           config, router, startup
│  │  ├─ config.rs
│  │  ├─ error.rs          AppError → HTTP response
│  │  ├─ auth/             webauthn, sessions, invites
│  │  ├─ cards/            handlers, repo, dto
│  │  ├─ sync/
│  │  ├─ brands/           search, Overpass client, refresh job
│  │  ├─ images/
│  │  └─ policy.rs         visibility/edit rules
│  ├─ migrations/
│  └─ tests/               integration tests (axum + temp SQLite)
├─ frontend/
│  ├─ src/
│  │  ├─ api/              fetch client + generated types
│  │  ├─ db/               Dexie schema, sync engine, outbox
│  │  ├─ features/{cards,scan,nearby,auth,household}/
│  │  └─ components/
│  └─ e2e/                 Playwright
├─ docs/DEPLOYMENT.md     runtime contract for the k8s manifests (owned outside this repo)
├─ .github/workflows/      ci.yml: lint, tests, e2e, image build + smoke test; push to GHCR on main/tags
├─ Dockerfile              multi-stage, cross-compiled per target arch
├─ docs/PLAN.md
└─ Makefile                dev, test, lint, build, gen-types, image
```

## 7. Milestones

Each milestone is test-first and ends with something usable on a phone.

### M0 — Skeleton & deployment path ✅
- Repo, `Makefile`, rustfmt/clippy, oxlint/Prettier, Tailwind, Vitest, Playwright (Chromium + WebKit).
- axum serves `/healthz`, `/readyz` and the embedded Vite build. Vite dev proxy for local development.
- `ts-rs` type generation wired into the build.
- Multi-arch image via `docker buildx` (`cargo-zigbuild`, no QEMU for the Rust step).
- `docs/DEPLOYMENT.md`: the contract the (externally owned) kustomize manifests must satisfy — port, probes, env vars, volume, single replica + `Recreate`, Litestream → R2, NetworkPolicy from `cloudflared`, Cloudflare WAF (`IT` only) and cache rules. Deployed with `kubectl apply -k`.
- GitHub Actions: lint + tests on PRs; on `main`, build and push `ghcr.io/<owner>/tessera:<sha>` for amd64 + arm64. Private GHCR package → `imagePullSecret` in the cluster.
- Security baseline: HSTS, CSP (with `wasm-unsafe-eval` for the scanner), Permissions-Policy, nosniff, frame denial, per-path cache headers.
  *(Session cookies, Origin check and rate limiting landed in M1 with the first mutating endpoints.)*
- **Done when:** the PWA installs from the public hostname on iPhone and Android and shows "hello" from the API; a pod restart on the other node architecture keeps the data.

### M1 — Auth & household ✅
- Migrations for users, passkeys, households, members, invites.
- Bootstrap: on first start, print a one-time admin registration link to the log.
- Passkey registration and login, sessions, logout.
- Invite link flow, so my wife registers and joins the household. Manage passkeys.
- Recovery: a `loyalty admin create-invite` CLI subcommand, run via `kubectl exec`, so a lost phone isn't a lockout.
- Brute-force protection: strict rate limits on auth endpoints; invite tokens are 256-bit, hashed, single-use, expiring.
- **Done when:** both of us can log in on our own phones.
- *As built:* every user belongs to exactly one household (`UNIQUE(user_id)`); only the owner can invite.
  WebAuthn ceremony state is kept in memory (single replica; restart = retry).
  Sessions are opaque cookies (`__Host-tessera_session`, SHA-256 stored), not JWTs, so logout and expiry are immediate.
  The Playwright suite runs real passkey ceremonies with Chromium's virtual authenticator.
  Security review follow-ups (done): recovery and passkey removal revoke other sessions, "sign out of all devices",
  IPv6 rate limiting per /64, at most 8 pending ceremonies per client, strong-entropy check for `TESSERA_BOOTSTRAP_TOKEN`,
  invite token stripped from the address bar, household invites valid 48h.
  Open: list/revoke pending household invites (low priority for two users).

### M2 — Cards core (online)
- Card CRUD with manual entry and barcode format picker. Validate number vs. format (e.g. EAN check digit).
- Fullscreen card view: `bwip-js` render, white background, Wake Lock, number shown as text below.
- List with search, favourites, "recently used". Front/back photos.
- **Done when:** I can add my real cards by hand and scan them at a till.

### M3 — Scanning & offline
- Camera scan with `zxing-wasm`, plus scan from an imported photo or screenshot.
- Dexie store, outbox, `/api/changes` pull sync, service worker caches the app shell and images.
- Offline indicator, retry with backoff, clear error messages.
- **Done when:** in airplane mode I can open and show every card, and edits sync after reconnecting.

### M4 — Sharing
- "Share with household" toggle on a card, owner-only unshare/delete.
- Visual badge for shared cards and who added them. Per-user favourites and usage.
- Policy unit tests for every visibility/edit combination.
- **Done when:** a card my wife shares appears on my phone after sync, and her private cards never do.

### M5 — Nearby
- Brand linking: search Wikidata/local brands when creating a card.
- Overpass client fetches locations for a brand inside Italy (`area["ISO3166-1"="IT"]`, from `TESSERA_REGION_ISO3166`). Weekly in-process refresh job, rate-limited, generic User-Agent.
- Store locations for linked brands are synced to IndexedDB.
- On app open: get one location fix, compute distance with haversine on the device, show a "Nearby" section (radius configurable, default 200 m).
- Custom places per card for shops missing from OSM.
- **Done when:** opening the app inside a supermarket puts its card at the top, including offline.

### M6 — Polish / later
- JSON export/import + encrypted backup; import from Stocard/CSV if exports allow it.
- Coupons/vouchers with expiry reminders.
- Map view (Leaflet + OSM tiles).
- Apple/Google Wallet pass export (experiment).
- Ops: documented Litestream restore drill, CI pipeline building and pushing the multi-arch image.

## 8. Testing strategy

- **Rust unit tests:** policy rules, barcode/number validation, sync cursor logic, Overpass response parsing (fixtures).
- **Rust integration tests:** each endpoint against a temp SQLite DB. Auth is tested via a test-only session helper; the WebAuthn ceremony is mocked.
- **Frontend unit tests:** Vitest + React Testing Library for components, sync engine, outbox, distance and nearby sorting.
- **E2E:** Playwright with mocked geolocation and a virtual WebAuthn authenticator. Flows: register → add card → show card → share → offline view.
- Coverage target: 80%+ on both sides.

## 9. Risks & mitigations

| Risk | Mitigation |
|---|---|
| iOS may evict PWA storage | Server is the source of truth; full re-sync on empty DB; "install to home screen" prompt |
| Passkey lost / phone replaced | Multiple passkeys per user + admin CLI recovery invite |
| App is publicly reachable | No Ingress; NetworkPolicy; WAF country allowlist; passkey-only auth; rate limits; security headers; security review before going live |
| Passkey RP ID tied to hostname | Choose the final public hostname before M1 and don't change it |
| SQLite on k8s (single writer, PVC bound to a node) | 1 replica + `Recreate`; Litestream to S3; prefer network-backed storage class so the pod can move between nodes |
| Cloudflare caching stale app/API | `Cache-Control: no-store` on `/api/*`, `no-cache` on `index.html` and the service worker; hashed assets cached immutably |
| OSM coverage gaps for some brands | Custom places per card |
| Overpass rate limits | Fetch per brand + region, cache in SQLite, weekly refresh, backoff |
| Barcode scan quality on low-end cameras | Manual entry always available; photo import fallback |
| Image uploads as an attack surface | MIME sniffing, size limits, re-encode, served only to authorized users |

## 10. Open questions

All resolved: name **Tessera** · Italy (`IT`) · existing Cloudflare Tunnel · k8s arm64 + amd64 with network storage · Litestream → R2 · GitHub + GHCR · Tailwind · hostname via env var · `kubectl apply -k` with manifests maintained by me outside this repo.

## 11. Naming

| Name | Meaning | Notes |
|---|---|---|
| **Tessera** *(chosen)* | Italian for card: "tessera fedeltà" is a loyalty card; also a mosaic tile | Short, meaningful in Italian, easy to type as a hostname |
| Fedeltà / Fidelio | From "carta fedeltà" | "Fidelio" is a Beethoven opera and an existing hotel software brand |
| Tasca | "Pocket" | Friendly, but less obvious |
| Portacarte | "Card holder" | Descriptive, long for a hostname |

The name drives: binary/crate name, image `ghcr.io/<owner>/<name>`, k8s namespace, env var prefix, PWA name and icon.
