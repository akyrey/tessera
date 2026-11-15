# Tessera developer tasks. Run `make help` for the list.

FRONTEND := frontend
BACKEND  := backend
IMAGE    ?= tessera:dev
PLATFORMS ?= linux/amd64,linux/arm64

DEV_ENV := TESSERA_PUBLIC_URL=http://localhost:5173 TESSERA_DATABASE_URL=sqlite://tessera-dev.db

.PHONY: help install dev-backend dev-frontend gen-types fmt lint test coverage e2e build image check

help: ## Show available targets
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-14s %s\n", $$1, $$2}'

install: ## Install frontend deps and Playwright browsers
	cd $(FRONTEND) && pnpm install --frozen-lockfile && pnpm exec playwright install chromium webkit

dev-backend: ## Run the API on :8080 (use with dev-frontend)
	cd $(BACKEND) && $(DEV_ENV) cargo run

dev-frontend: ## Run Vite on :5173, proxying /api to :8080
	cd $(FRONTEND) && pnpm dev

gen-types: ## Regenerate TypeScript types from Rust (ts-rs)
	cd $(BACKEND) && cargo test --quiet --lib

fmt: ## Format Rust and TypeScript
	cd $(BACKEND) && cargo fmt
	cd $(FRONTEND) && pnpm format

lint: ## Lint and typecheck everything
	cd $(BACKEND) && cargo fmt --check && cargo clippy --all-targets --locked -- -D warnings
	cd $(FRONTEND) && pnpm format:check && pnpm lint && pnpm typecheck

test: ## Unit and integration tests
	cd $(BACKEND) && cargo test --locked
	cd $(FRONTEND) && pnpm test

coverage: ## Frontend coverage (80% threshold enforced)
	cd $(FRONTEND) && pnpm test:coverage

e2e: ## Build the frontend, then run Playwright against the Rust server
	cd $(FRONTEND) && pnpm build && pnpm e2e

build: ## Release binary with the frontend embedded
	cd $(FRONTEND) && pnpm build
	cd $(BACKEND) && cargo build --release --locked

image: ## Multi-arch container image
	docker buildx build --platform $(PLATFORMS) -t $(IMAGE) .

check: lint test coverage e2e ## Everything CI runs
