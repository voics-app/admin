# Architecture

Clean Architecture inside `bff/src`: `domain` (pure rules, no IO) <- `application` (use cases and ports) <- `infrastructure` (adapters) and `interface` (HTTP), wired in `bootstrap.rs`. Dependencies point inward only.

```mermaid
C4Component
    title Admin BFF components
    Person(admin, "Operator", "Behind Cloudflare Access")
    Container_Boundary(bff, "admin-bff") {
        Component(http, "interface/http", "axum", "Routes, DTOs, auth middleware, embedded SPA")
        Component(app, "application", "use cases + ports", "Overview, usage, voice regions, health, T&S, flags, audit, ingest")
        Component(domain, "domain", "pure Rust", "Rollups, allow-list, flags, regions, health rules")
        Component(bus, "infrastructure/bus", "rskafka", "Decodes P6 envelopes content-free, pumps partitions")
        Component(pg, "infrastructure/postgres", "sqlx", "Rollups, checkpoints, flags, audit")
        Component(adapters, "infrastructure/{http,etcd,victoria,cloudflare}", "reqwest", "api, gateway, etcd, VictoriaMetrics, Access JWKS")
    }
    ContainerDb(db, "discordy_admin", "Postgres")
    ContainerQueue(rp, "Redpanda", "Kafka API")
    Container_Ext(up, "api, gateway, etcd, VictoriaMetrics", "Internal services")
    Rel(admin, http, "HTTPS")
    Rel(http, app, "calls")
    Rel(app, domain, "uses")
    Rel(bus, app, "ingest use case")
    Rel(pg, db, "SQL")
    Rel(bus, rp, "consume")
    Rel(adapters, up, "HTTP")
```

## Usage aggregation

One consumer task per partition decodes only the envelope header fields (type, timestamp, ids) and classifies them into counters; message bodies are never deserialised. Counts accumulate into a delta, and the delta and the partition offset are committed in one Postgres transaction, so a crash can neither lose nor double count. A bounded memory of recent event ids drops redeliveries. Rollups are kept at minute, hour and day granularity and pruned by age.

## Auth

The `require_admin` middleware runs the `AuthenticateAdmin` use case: verify the Cloudflare Access JWT through the `AccessTokenVerifier` port (JWKS cached), then check the email against the domain `AdminAllowlist`. The dev header is honoured only when `DISCORDY_ENV=local`.

## Web

`web/` is a Vite SPA with one typed API client, a `useAsync` loader, Recharts-based time-series components that follow the data-viz rules (fixed categorical order, one axis, legend, table view, status as icon plus label), and Nightglass design tokens shared with the client. `rust-embed` bakes `web/dist` into the binary; unknown non-file paths fall back to `index.html`.
