# syntax=docker/dockerfile:1.7

# --- web: build the SPA ---
FROM node:24-alpine AS web
WORKDIR /src/web
RUN corepack enable
COPY web/package.json web/pnpm-lock.yaml ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm build

# --- chef: cargo-chef builds the BFF dependencies once, from a recipe of the manifests only ---
FROM rust:1.99-slim-bookworm AS chef
ARG CHEF_VERSION=0.1.78
RUN --mount=type=cache,target=/usr/local/cargo/registry     cargo install cargo-chef --locked --version "${CHEF_VERSION}"
WORKDIR /src/bff

FROM chef AS planner
COPY bff/ ./
RUN cargo chef prepare --recipe-path recipe.json

# --- bff: compile the server with the SPA embedded (rust-embed reads ../web/dist) ---
FROM chef AS bff
COPY --from=planner /src/bff/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry     --mount=type=cache,target=/usr/local/cargo/git     cargo chef cook --release --locked --recipe-path recipe.json
COPY bff/ ./
COPY --from=web /src/web/dist /src/web/dist
RUN --mount=type=cache,target=/usr/local/cargo/registry     --mount=type=cache,target=/usr/local/cargo/git     cargo build --release --locked --bin admin-bff

# --- runtime ---
FROM gcr.io/distroless/cc-debian12:nonroot
ARG VERSION=0.1.0
ARG REVISION=unknown
LABEL org.opencontainers.image.title="discordy-admin" \
      org.opencontainers.image.description="Discordy admin portal: Rust BFF with the React SPA embedded" \
      org.opencontainers.image.source="https://github.com/discordy-voics/admin" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}" \
      org.opencontainers.image.licenses="UNLICENSED" \
      org.opencontainers.image.vendor="discordy-voics"
COPY --from=bff /src/bff/target/release/admin-bff /usr/local/bin/admin-bff
USER nonroot:nonroot
EXPOSE 8090
ENTRYPOINT ["/usr/local/bin/admin-bff"]
