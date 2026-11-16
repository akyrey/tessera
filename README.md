# Tessera

Self-hosted loyalty-card wallet: Rust (axum + SQLite) backend serving a React PWA.
Installable on iPhone and Android, works offline, cards shareable within a household.

- Plan and milestones: [docs/PLAN.md](docs/PLAN.md)
- What the Kubernetes manifests must provide: [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md)

## Development

Requires Rust (stable), Node ≥ 22 and pnpm 12 (pinned via `packageManager`).

```sh
make install        # frontend deps + Playwright browsers
make dev-backend    # API on :8080
make dev-frontend   # Vite on :5173, proxies /api → :8080
make check          # lint, tests, coverage, e2e (what CI runs)
make image          # multi-arch container image
```

First run: start the backend and open the first-admin link it logs (`/join#…`).
Other admin tasks: `cargo run -- admin --help` (in the cluster: `kubectl exec … -- /tessera admin …`).

Rust DTOs annotated with `#[ts(export)]` generate TypeScript into `frontend/src/api/generated/` when the backend tests run (`make gen-types`).
Those files are committed, and CI fails if they drift.

## License

[MIT](LICENSE)
