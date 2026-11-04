//! Voics admin BFF.
//!
//! Layers (dependencies point inward only):
//! - `domain`: pure usage/rollup/flag/voice rules, no IO.
//! - `application`: use cases and the ports they need.
//! - `infrastructure`: adapters (Postgres, Redpanda, etcd, VictoriaMetrics, internal HTTP, Cloudflare Access).
//! - `interface`: HTTP routes, DTOs, auth middleware, SPA serving.
//!
//! The composition root is [`bootstrap`], configured by [`config`] and started from `main.rs`.

pub mod application;
pub mod bootstrap;
pub mod config;
pub mod domain;
pub mod infrastructure;
pub mod interface;
pub mod telemetry;
