# syntax=docker/dockerfile:1.7
#
# Multi-arch image: `docker buildx build --platform linux/amd64,linux/arm64 .`
# Both build stages run natively on the build host ($BUILDPLATFORM). Rust is
# cross-compiled to a static musl binary with cargo-zigbuild, so no QEMU emulation.

ARG NODE_VERSION=24
ARG RUST_VERSION=1
ARG ZIG_VERSION=0.13.0

# ---------- frontend ----------
FROM --platform=$BUILDPLATFORM node:${NODE_VERSION}-bookworm-slim AS frontend
WORKDIR /src/frontend
RUN corepack enable
COPY frontend/package.json frontend/pnpm-lock.yaml ./
RUN --mount=type=cache,id=pnpm-store,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile
COPY frontend/ ./
RUN pnpm build

# ---------- backend ----------
FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-bookworm AS backend
ARG ZIG_VERSION
ARG TARGETARCH
RUN apt-get update \
    && apt-get install -y --no-install-recommends python3-pip \
    && rm -rf /var/lib/apt/lists/* \
    && pip3 install --break-system-packages --no-cache-dir "ziglang==${ZIG_VERSION}" \
    && cargo install --locked cargo-zigbuild \
    && rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
WORKDIR /src/backend
COPY backend/ ./
# rust-embed bakes the frontend into the release binary at compile time.
COPY --from=frontend /src/frontend/dist /src/frontend/dist
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-target-${TARGETARCH},target=/src/backend/target \
    case "${TARGETARCH}" in \
      amd64) TARGET=x86_64-unknown-linux-musl ;; \
      arm64) TARGET=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported TARGETARCH=${TARGETARCH}" >&2; exit 1 ;; \
    esac \
    && cargo zigbuild --release --locked --target "${TARGET}" \
    && install -D "target/${TARGET}/release/tessera" /out/tessera \
    && mkdir -p /out/data

# ---------- runtime ----------
FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=backend /out/tessera /tessera
# Owned by the distroless `nonroot` user (65532) so fresh volumes are writable.
COPY --from=backend --chown=65532:65532 /out/data /data
ENV TESSERA_LISTEN_ADDR=0.0.0.0:8080 \
    TESSERA_DATABASE_URL=sqlite:///data/tessera.db
EXPOSE 8080
USER nonroot:nonroot
ENTRYPOINT ["/tessera"]
