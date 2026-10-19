/// Additive metrics: rollups sum them per bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CounterMetric {
    /// Every bus event, dimension = event type.
    Events,
    /// Messages, dimension = delivery mode.
    Messages,
    /// Voice participant-seconds, dimension = region.
    VoiceSeconds,
}

/// Point-in-time metrics: rollups keep the peak per bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GaugeMetric {
    /// Gateway sessions online, dimension = gateway node.
    SessionsOnline,
    /// Users connected to a voice node, dimension = voice node.
    VoiceUsers,
    /// Calls hosted on a voice node, dimension = voice node.
    ActiveCalls,
    /// Registered users platform-wide, dimension = "" (sampled from api stats).
    UsersTotal,
}

impl CounterMetric {
    pub fn as_str(self) -> &'static str {
        match self {
            CounterMetric::Events => "events",
            CounterMetric::Messages => "messages",
            CounterMetric::VoiceSeconds => "voice_seconds",
        }
    }
}

impl GaugeMetric {
    pub fn as_str(self) -> &'static str {
        match self {
            GaugeMetric::SessionsOnline => "sessions_online",
            GaugeMetric::VoiceUsers => "voice_users",
            GaugeMetric::ActiveCalls => "active_calls",
            GaugeMetric::UsersTotal => "users_total",
        }
    }
}
