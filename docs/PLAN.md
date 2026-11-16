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
| CI/CD | GitHub Actions → Docker Hub (multi-arch image) | Repo lives on GitHub |
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
| `GET /api/changes?since=` | Pull sync: cursor + full snapshot of visible cards, or `null` when unchanged |
| `GET /api/cards` | Cards visible to me, with my favourite/usage and photo ids (M2; superseded by `/api/changes` for offline use in M3) |
| `PUT /api/cards/{id}` · `DELETE /api/cards/{id}` | Idempotent upsert (client-chosen UUIDv7) / soft delete leaving a tombstone |
| `PUT /api/cards/{id}/places` | Replace the hand-marked shops only |
| `PUT /api/cards/{id}/sharing` | Share with / unshare from the owner's household (owner only) |
| `PUT /api/cards/{id}/prefs` · `POST /api/cards/{id}/use` | Per-user favourite; usage bump for "recently used" |
| `PUT/DELETE /api/cards/{id}/images/{front\|back}` · `GET /api/images/{id}` | Raw-body photo upload (≤ 512 KB, type checked against magic bytes), authorized download |
| `GET /api/brands/search?q=` | Linked brands plus Wikidata candidates |
| `POST /api/brands` | Link a brand (wikidata id; name fetched from Wikidata) and trigger a location fetch |
| `GET /api/locations?version=` | Radius and shop points of the brands on visible cards, or `null` when unchanged |

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
├─ .github/workflows/      ci.yml: lint, tests, e2e, image build + smoke test; push to Docker Hub on main/tags
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
- GitHub Actions: lint + tests on PRs; on `main`, build and push `docker.io/<user>/tessera:<sha>` for amd64 + arm64. Private Docker Hub repo → `imagePullSecret` in the cluster.
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

### M2 — Cards core (online) ✅
- Card CRUD with manual entry and barcode format picker. Validate number vs. format (e.g. EAN check digit).
- Fullscreen card view: `bwip-js` render, white background, Wake Lock, number shown as text below.
- List with search, favourites, "recently used". Front/back photos.
- **Done when:** I can add my real cards by hand and scan them at a till.
- *As built:* formats EAN-13/8, UPC-A, Code 128, Code 39, ITF, Codabar, QR, PDF417, Aztec, Data Matrix, and
  "number only". Validation lives in `cards/barcode.rs` (authoritative) and is mirrored in
  `features/cards/barcodeFormats.ts` for instant feedback; numeric formats drop spaces, Code 39/Codabar are upper-cased.
  Every write takes a number from `change_counter` (first statement of the transaction, so it also takes the SQLite
  write lock), ready for M3 pull sync. Deleting leaves a content-free tombstone and drops photos and prefs; a replayed
  `PUT` cannot resurrect it. Access goes through `cards/policy.rs`; cards a user cannot see answer 404.
  Photos are downscaled on the phone (≤ 1280 px, WebP or JPEG on Safari, EXIF dropped) and stored as BLOBs;
  each replacement gets a new id, so image responses are cached as `immutable`.
  Barcodes are rendered as SVG `data:` images (lazy-loaded `bwip-js`). The web cannot raise screen brightness:
  turn it up by hand at the till.
  E2E: the passkey journey adds, shows, favourites, edits and deletes a card on Chromium; `cards.spec.ts` renders
  barcodes and re-encodes photos on both Chromium and WebKit with a stubbed JSON API.
  Security review follow-ups (done): per-user caps (500 cards, 100 MB of photos → `409 quota_exceeded`),
  `410 card_deleted` when the owner saves a deleted card, JSON envelope for oversized uploads, tombstones drop the
  household link. Open: optimistic concurrency (`updatedAt` precondition → 409) before sharing makes concurrent
  edits likely (M3/M4); resolve the viewer's household inside the write transaction (M4); photos stay in the
  browser cache after deletion (`private, immutable`), accepted for personal phones; uploads are checked by magic
  bytes, not fully decoded.

### M3 — Scanning & offline ✅
- Camera scan with `zxing-wasm`, plus scan from an imported photo or screenshot.
- Dexie store, outbox, `/api/changes` pull sync, service worker caches the app shell and images.
- Offline indicator, retry with backoff, clear error messages.
- **Done when:** in airplane mode I can open and show every card, and edits sync after reconnecting.
- *As built:* `GET /api/changes?since=` returns the cursor plus either `null` (nothing changed) or a **full snapshot**
  of visible cards: with two users this is smaller and simpler than deltas, and deletions/unsharing need no extra
  bookkeeping. Joining a household bumps the cursor. The UI reads only IndexedDB (Dexie); every edit is applied
  locally and queued in an outbox (repeated saves coalesce, a delete drops pending edits). The engine flushes the
  outbox, then pulls, keeping local versions of cards with unsent changes. Network and 5xx errors retry with
  backoff (2 s → 1 min) and on `online`/visibility; refused changes (4xx) are dropped and shown once; 401 re-checks
  the session. The signed-in user is cached for offline starts; signing out, an expired session or another account
  wipes the device copy. Photos are queued as bytes (Safari private mode cannot store Blobs in IndexedDB) and
  previewed from a local `data:` URL until synced; the service worker caches `/api/images/*` (cache-first, ids are
  immutable) and precaches the app shell, bwip-js and the scanner WASM.
  Scanning: `zxing-wasm` reader with the WASM self-hosted (the default CDN is blocked by the CSP); images are decoded
  by the browser onto white first (HEIC photos, transparent screenshots). Camera scanning reads 4 frames/s.
  E2E: offline reload, offline edit and sync-after-reconnect on Chromium; scan-from-photo and offline app shell on
  both engines. Found on the way: a duplicate precache entry silently disabled the service worker, and a late
  "signed out" answer could overwrite a fresh sign-in (now cancelled).
  Security review follow-ups (done): a save is never coalesced into the entry being sent (lost edit); wipes bump a
  generation so in-flight pulls cannot write afterwards, sync stops before sign-out, and the signed-in UI waits until
  the device data belongs to the current user (data of an unknown or other owner is wiped); cursors are per user;
  a refused change forces a full snapshot (no ghost cards); a change failing with 5xx is given up after 8 tries;
  a 5xx from `/api/me` also falls back to the cached user; an expired session keeps unsent changes for the same
  user; sign-out warns about unsynced changes; the camera is released if closed during the permission prompt;
  scanner failures are shown and the WASM load is retried. Open: after a deploy, a page still running the previous
  build cannot lazy-load its old bwip-js/scanner chunks (the number stays visible; reopening the app fixes it).

### M4 — Sharing ✅
- "Share with household" toggle on a card, owner-only unshare/delete.
- Visual badge for shared cards and who added them. Per-user favourites and usage.
- Policy unit tests for every visibility/edit combination.
- **Done when:** a card my wife shares appears on my phone after sync, and her private cards never do.
- *As built:* `PUT /api/cards/{id}/sharing {shared}` (owner only: 403 for members, 404 for others) sets the card's
  `household_id`; cards carry `shared` and `ownerName`. Members can edit shared cards and add photos (counted in the
  owner's quota) but cannot delete or unshare them. The share switch appears only for the owner and only when the
  household has more than one member; sharing is queued in the outbox like every other change. The viewer's
  household is now read inside each write transaction. Concurrent edits stay last-write-wins (two users).
  E2E: Alice shares a card, Bea sees "Shared by Alice" with the barcode, Alice unshares, Bea's wallet empties.

### M5 — Nearby ✅
- Brand linking: search Wikidata/local brands when creating a card.
- Overpass client fetches locations for a brand inside Italy (`area["ISO3166-1"="IT"]`, from `TESSERA_REGION_ISO3166`). Weekly in-process refresh job, rate-limited, generic User-Agent.
- Store locations for linked brands are synced to IndexedDB.
- On app open: get one location fix, compute distance with haversine on the device, show a "Nearby" section (radius configurable, default 200 m).
- Custom places per card for shops missing from OSM.
- **Done when:** opening the app inside a supermarket puts its card at the top, including offline.
- *As built:* the card form links a **brand**: `GET /api/brands/search?q=` returns brands already linked plus
  Wikidata candidates (`wbsearchentities`, Italian labels; still answers with local brands when Wikidata is down);
  `POST /api/brands {wikidataId}` creates the brand with its name **from Wikidata** (not from the client) and
  starts an Overpass fetch in the background. Overpass: `nwr["brand:wikidata"=Q…](area ISO3166-1=IT)`, `out center`,
  one query at a time with ≥ 5 s between queries, generic User-Agent; the job refreshes brands used by a live card
  every `TESSERA_STORE_REFRESH_INTERVAL` and backs off 6 h after a failure. Both endpoints have their own rate limit
  (30/min per client). `GET /api/locations?version=` returns the radius and `[lat, lon]` points of every brand on a
  visible card (or `null` when the version is current); the client stores them in IndexedDB on each sync.
  Custom places: up to 20 per card, saved with the card ("I am at this shop" on the card view).
  On the list page the phone takes one location fix (automatically only if already allowed, otherwise from a button),
  computes haversine distances on the device and shows a "Nearby" section, closest first.
  TLS: reqwest 0.13 + rustls with the `ring` provider (cross-compiles with zig; roots from the image's CA bundle).
  Tests use a fake Wikidata/Overpass (axum on a random port); E2E points both at an unreachable local port.
  E2E: a hand-marked shop brings its card to the top there and not elsewhere (Chromium); nearby by brand on both
  engines with a stubbed API and a mocked position.
  Security review follow-ups (done): linking a brand no longer fetches anything; the first Overpass fetch starts
  when a card uses the brand (deduplicated while queued) and the brand table is capped at 1000; an Overpass `remark`
  (partial result) or an empty answer keeps the stored shops; responses are size-capped (1 MB Wikidata, 32 MB
  Overpass, `[maxsize]` in the query) and redirects are not followed; inserts are batched; stored Wikidata ids are
  re-validated before building a query; brand search lists only brands on the caller's visible cards; places have
  their own `PUT /api/cards/{id}/places` (card saves without `places` keep them, so a places edit never overwrites
  other fields); a failing `/api/locations` no longer fails the card sync. Open: Wikidata items are not checked to
  be brands (an unrelated item just finds no shops); `/api/locations` can reach a few MB for many large chains.


### M6 — Polish / later ✅ (partly)
- JSON export/import + encrypted backup; import from Stocard/CSV if exports allow it.
- Coupons/vouchers with expiry reminders.
- Map view (Leaflet + OSM tiles).
- Apple/Google Wallet pass export (experiment).
- Ops: documented Litestream restore drill, CI pipeline building and pushing the multi-arch image.
- *As built:*
  - **Expiry:** cards take an optional last valid day (`expires_on`, `YYYY-MM-DD`). The list shows an "Expiring soon"
    group (within 7 days, closest first) and marks expired cards; the card view shows "Expires in N days". The reminder
    is in-app only: Web Push needs a push service and VAPID keys, and iOS only delivers it to installed PWAs.
  - **Export/import:** Settings → "Export cards" downloads a versioned JSON file (`tessera-backup` v1) with every card
    the user owns, including photos as `data:` URLs. Import validates the file with zod (≤ 50 MB, ≤ 500 cards, photo
    types checked) and adds the cards through the outbox, so it works offline. Brands are not restored; the brand name is
    kept for reference and is linked again from the card form.
  - **Map:** `/map` (linked from the list) shows the shops of the user's cards with Leaflet and OSM tiles. The CSP
    allows `https://tile.openstreetmap.org`. Tiles need a connection; the shop list itself works offline.
  - **Ops:** a restore drill is in `DEPLOYMENT.md`. The CI pipeline that builds and pushes the multi-arch image already
    existed since M0 (`.github/workflows/ci.yml`).
  - **Not done:**
    - The export is not encrypted. Litestream covers the server, and a passphrase-encrypted export can be added later.
    - Stocard has no export, so there is nothing to import from it. A CSV import was not built.
    - Apple/Google Wallet passes are not built: they need an Apple Developer account with a Pass Type ID certificate,
      and a Google Wallet issuer account.

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

All resolved: name **Tessera** · Italy (`IT`) · existing Cloudflare Tunnel · k8s arm64 + amd64 with network storage · Litestream → R2 · GitHub + Docker Hub · Tailwind · hostname via env var · `kubectl apply -k` with manifests maintained by me outside this repo.

## 11. Naming

| Name | Meaning | Notes |
|---|---|---|
| **Tessera** *(chosen)* | Italian for card: "tessera fedeltà" is a loyalty card; also a mosaic tile | Short, meaningful in Italian, easy to type as a hostname |
| Fedeltà / Fidelio | From "carta fedeltà" | "Fidelio" is a Beethoven opera and an existing hotel software brand |
| Tasca | "Pocket" | Friendly, but less obvious |
| Portacarte | "Card holder" | Descriptive, long for a hostname |

The name drives: binary/crate name, image `docker.io/<user>/<name>`, k8s namespace, env var prefix, PWA name and icon.
