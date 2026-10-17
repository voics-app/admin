# syntax=docker/dockerfile:1.7

# --- web: build the SPA ---
FROM node:24-alpine AS web
WORKDIR /src/web
RUN corepack enable
COPY web/package.json web/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY web/ ./
RUN pnpm build

# --- bff: compile the server with the SPA embedded (rust-embed reads ../web/dist) ---
FROM rust:1-bookworm AS bff
WORKDIR /src
COPY bff/Cargo.toml bff/Cargo.lock bff/
# Warm the dependency cache with a stub crate before the real sources arrive.
RUN mkdir -p bff/src web/dist && echo "fn main() {}" > bff/src/main.rs && echo "" > bff/src/lib.rs \
    && touch bff/build.rs \
    && cd bff && cargo build --release --locked 2>/dev/null || true
COPY bff/ bff/
COPY --from=web /src/web/dist web/dist
RUN cd bff && touch src/main.rs src/lib.rs && cargo build --release --locked --bin admin-bff

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
