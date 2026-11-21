# Voics admin

Channel reports in Reports show a distinct channel target, server-captured channel
metadata and the server owner at filing. They do not include messages, attachments,
topics or member lists, and resolving a report does not change a channel or suspend
its owner. This supports the age-restricted channel gate in client#76.

Operations portal for Voics: a Rust backend-for-frontend (`bff/`, axum + sqlx) and a React SPA (`web/`, Vite + Recharts). The SPA is embedded in the BFF binary, so the whole portal ships as one image listening on port 8090.

Pages: Overview, Usage, Voice regions (drain/resume nodes), Platform health, Trust & Safety (user and guild lookup, suspension), Reports, Send notification (`/notify`: a push notification by hand to one user or everyone), Feature flags, Audit log.

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
| `NOTIFIER_INTERNAL_URL` | no | notifier base URL (`http://notifier.voics-apps:8085`), for Send notification. Without it the page answers that sending is not configured |
| `VICTORIAMETRICS_URL` | yes | PromQL endpoint |
| `ETCD_ENDPOINTS` | yes | comma-separated etcd endpoints (voice node registry) |
| `ADMIN_ALLOWED_EMAILS` | yes | comma-separated allow-list, the second gate after Cloudflare Access |
| `CF_ACCESS_TEAM_DOMAIN` | production | Cloudflare Zero Trust team name, used to fetch the JWKS |
| `CF_ACCESS_AUD` | production | audience tag of the Access application (legacy single-host mode) |
| `CF_ACCESS_HOST_AUDIENCES` | optional | comma-separated exact `hostname=audience` pairs; when set, unknown hosts are rejected and each host requires its own audience |

## Auth

Every `/admin-api` request needs a valid `Cf-Access-Jwt-Assertion` (RS256, verified against `https://<team>.cloudflareaccess.com/cdn-cgi/access/certs`, issuer and audience checked) whose email is on `ADMIN_ALLOWED_EMAILS`. Failures return 401 or 403 with the platform `{code, message}` envelope. Mutating actions (drain, suspend, flags, sending a notification) are written to the audit log with the admin's email.

Ops endpoints (no auth): `/healthz`, `/readyz`, `/metrics`.

## Notifier backlog and drops

Platform health includes **Dropped pushes (5m)** by provider, **Push queue** and **Push sends in
flight** by notifier instance. These use the existing authenticated
`GET /admin-api/platform/health/series?panel=...&range=...` response shape
`{panel, range, series:[{label, points:[[timestamp,value]]}]`. Added panel keys are `push_drops`,
`push_queue` and `push_in_flight`. No token, recipient or message content is queried.

`push_drops` uses `sum by (provider) (increase(notifier_pushes_total{outcome="dropped"}[5m]))`.
Each chart point is the estimated drop count for the preceding five minutes; it handles counter
resets and is not a count over the entire selected range. Queue and in-flight charts use the
corresponding `notifier_queue_depth` and `notifier_in_flight` gauges. In-flight includes retry waits.
The notifier also exports configured limits as `notifier_queue_capacity` and
`notifier_in_flight_limit`; compare these when investigating saturation. An empty series means
no samples, not zero loss. The existing notifier scrape job must be reachable in VictoriaMetrics.
No provider credentials or new BFF configuration are required by these read-only panels.

## Discovery listing reports

The Reports queue also accepts `kind: guild` from discovery. It identifies the
guild and owner at filing, and labels evidence as a public listing snapshot.
Only the public name, description, category and tags are captured; no private
channels, members or messages are copied. Resolve/dismiss uses the existing
audited operator flow and does not remove the listing. No new BFF route,
credential or configuration is required; deploy with the paired api#45 change.

## Pending owner actions

- Create the Cloudflare Access application for the admin hostname and set `CF_ACCESS_TEAM_DOMAIN` and `CF_ACCESS_AUD` as secrets (needs the Cloudflare token).
- Create the `discordy_admin` database and role in CloudNativePG (gitops).
- Publish the repository to the GitHub org so CI can call the reusable workflows.
- The api and gateway must expose the admin endpoints this BFF calls (P4: `GET /internal/v1/admin/stats`, user and guild search, suspend; gateway stats).

## Git hooks

Tests run locally, not in CI. Install [lefthook](https://github.com/evilmartians/lefthook) once and run `make hooks` (or `lefthook install`) to enable the pre-push hook in `lefthook.yml`: it runs this repository's lint and unit tests before every push. CI repeats only the slim gate on pull requests. Skip once with `LEFTHOOK=0 git push`.
