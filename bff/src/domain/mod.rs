//! Pure domain model. Nothing in here performs IO or depends on a framework.

pub mod admin_allowlist;
pub mod audit;
pub mod delivery_mode;
pub mod feature_flag;
pub mod granularity;
pub mod metric;
pub mod observed_event;
pub mod recent_ids;
pub mod rollup;
pub mod service_health;
pub mod signups;
pub mod usage_signal;
pub mod voice_region;
