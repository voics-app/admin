use chrono::{DateTime, Utc};

/// The minimal, content-free view of a bus event that the usage aggregator needs.
///
/// Adapters decode the P6 envelope into this shape and drop everything else, so no
/// message content ever reaches the domain.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservedEvent {
    pub id: String,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    /// Users this event proves were active (authors, typers, reactors, ...).
    pub actor_ids: Vec<String>,
    pub facts: EventFacts,
}

/// Type-specific counters carried by an event.
#[derive(Debug, Clone, PartialEq)]
pub enum EventFacts {
    /// A standard-mode message was persisted.
    MessageCreated,
    /// Periodic counters published by a gateway or voice node.
    UsageTick(UsageTick),
    /// One client voice-quality report.
    VoiceQuality(VoiceQualitySample),
    /// Any other event: only counted by type.
    Other,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UsageTick {
    pub node_id: String,
    pub interval_seconds: u32,
    pub sessions_online: Option<u64>,
    pub ephemeral_relayed: u64,
    pub p2p_signaled: u64,
    pub voice_users: Option<u64>,
    pub active_calls: Option<u64>,
    pub voice_region: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceQualitySample {
    pub region: String,
    pub jitter_ms: f64,
    pub loss_pct: f64,
    pub rtt_ms: f64,
    pub bitrate_kbps: f64,
}
