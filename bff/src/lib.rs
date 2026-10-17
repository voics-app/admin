//! Discordy admin BFF.
//!
//! Layers (dependencies point inward only):
//! - `domain`: pure usage/rollup/flag/voice rules, no IO.
//! - `application`: use cases and the ports they need.
//! - `infrastructure`: adapters (Postgres, Redpanda, etcd, VictoriaMetrics, internal HTTP, Cloudflare Access).
//! - `interface`: HTTP routes, DTOs, auth middleware, SPA serving.
//!
//! The composition root lives in `main.rs` / [`bootstrap`].

pub mod application;
pub mod domain;
pub mod infrastructure;
