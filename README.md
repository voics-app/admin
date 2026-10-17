# Discordy admin

Operations portal for Discordy: a Rust backend-for-frontend (`bff/`, axum + sqlx) and a React SPA (`web/`, Vite + Recharts). The SPA is embedded in the BFF binary, so the whole portal ships as one image listening on port 8090.

Pages: Overview, Usage, Voice regions (drain/resume nodes), Platform health, Trust & Safety (user and guild lookup, suspension), Feature flags, Audit log.

The BFF counts events from every Redpanda topic into minute, hour and day rollups in its own database (`discordy_admin`). It reads counters only and never stores message content.

## Run locally

Prerequisites: Rust 1.99, Node 24, pnpm, and the compose dev stack (Postgres, Redpanda, etcd, VictoriaMetrics) from `gitops/local`.

```sh
cp .env.example .env           # dev-only values
set -a; . ./.env; set +a
make dev                       # BFF on :8090, Vite on :5174 (proxies /admin-api and sends X-Dev-Admin-Email)
```

With `DISCORDY_ENV=local` the BFF accepts `X-Dev-Admin-Email` in place of a Cloudflare Access token. In production that header is ignored.

## Test and lint

```sh
make test    # cargo test (Postgres via testcontainers, HTTP via wiremock) + vitest
make lint    # cargo fmt --check, clippy -D warnings, eslint, tsc --noEmit
make docker  # single image with the SPA embedded
```

## Configuration

All configuration is environment variables, validated at startup (all problems are reported together).

| Variable | Required | Description |
|---|---|---|
| `DISCORDY_ENV` | yes | `local` or `production` |
| `LOG_LEVEL` | no | default `info` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | no | OTLP/HTTP traces endpoint |
| `INTERNAL_SERVICE_TOKEN` | yes | sent as `X-Internal-Token` to api and gateway internal APIs |
| `KAFKA_BROKERS` | yes | comma-separated Redpanda brokers |
| `ADMIN_DATABASE_URL` | yes | Postgres URL of `discordy_admin` (migrations run at startup) |
| `API_INTERNAL_URL` | yes | api service base URL (P4 admin endpoints) |
| `GATEWAY_INTERNAL_URL` | yes | gateway base URL (stats) |
| `VICTORIAMETRICS_URL` | yes | PromQL endpoint |
| `ETCD_ENDPOINTS` | yes | comma-separated etcd endpoints (voice node registry) |
| `ADMIN_ALLOWED_EMAILS` | yes | comma-separated allow-list, the second gate after Cloudflare Access |
| `CF_ACCESS_TEAM_DOMAIN` | production | Cloudflare Zero Trust team name, used to fetch the JWKS |
| `CF_ACCESS_AUD` | production | audience tag of the Access application |

## Auth

Every `/admin-api` request needs a valid `Cf-Access-Jwt-Assertion` (RS256, verified against `https://<team>.cloudflareaccess.com/cdn-cgi/access/certs`, issuer and audience checked) whose email is on `ADMIN_ALLOWED_EMAILS`. Failures return 401 or 403 with the platform `{code, message}` envelope. Mutating actions (drain, suspend, flags) are written to the audit log with the admin's email.

Ops endpoints (no auth): `/healthz`, `/readyz`, `/metrics`.

## Pending owner actions

- Create the Cloudflare Access application for the admin hostname and set `CF_ACCESS_TEAM_DOMAIN` and `CF_ACCESS_AUD` as secrets (needs the Cloudflare token).
- Create the `discordy_admin` database and role in CloudNativePG (gitops).
- Publish the repository to the GitHub org so CI can call the reusable workflows.
- The api and gateway must expose the admin endpoints this BFF calls (P4: `GET /internal/v1/admin/stats`, user and guild search, suspend; gateway stats).
