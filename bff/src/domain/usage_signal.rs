use super::delivery_mode::DeliveryMode;
use super::metric::{CounterMetric, GaugeMetric};
use super::observed_event::{EventFacts, ObservedEvent, UsageTick, VoiceQualitySample};

/// One countable fact derived from an event.
#[derive(Debug, Clone, PartialEq)]
pub enum UsageSignal {
    Count { metric: CounterMetric, dimension: String, amount: i64 },
    Gauge { metric: GaugeMetric, dimension: String, value: f64 },
    ActiveUser { user_id: String },
    Quality(VoiceQualitySample),
}

/// Region used when a voice node does not report one.
pub const UNKNOWN_REGION: &str = "unknown";

/// Derives every usage signal an event contributes. Pure and deterministic.
pub fn signals_for(event: &ObservedEvent) -> Vec<UsageSignal> {
    let mut signals = vec![count(CounterMetric::Events, &event.event_type, 1)];
    signals.extend(active_users(&event.actor_ids));
    match &event.facts {
        EventFacts::MessageCreated => {
            signals.push(count(CounterMetric::Messages, DeliveryMode::Standard.as_str(), 1));
        }
        EventFacts::UsageTick(tick) => signals.extend(tick_signals(tick)),
        EventFacts::VoiceQuality(sample) => signals.push(UsageSignal::Quality(sample.clone())),
        EventFacts::Other => {}
    }
    signals
}

fn count(metric: CounterMetric, dimension: &str, amount: i64) -> UsageSignal {
    UsageSignal::Count { metric, dimension: dimension.to_owned(), amount }
}

fn active_users(actor_ids: &[String]) -> Vec<UsageSignal> {
    let mut unique: Vec<&String> = actor_ids.iter().collect();
    unique.sort();
    unique.dedup();
    unique.into_iter().map(|id| UsageSignal::ActiveUser { user_id: id.clone() }).collect()
}

fn tick_signals(tick: &UsageTick) -> Vec<UsageSignal> {
    let mut signals = Vec::new();
    let private_modes = [
        (DeliveryMode::Ephemeral, tick.ephemeral_relayed),
        (DeliveryMode::P2p, tick.p2p_signaled),
    ];
    for (mode, amount) in private_modes {
        if amount > 0 {
            signals.push(count(CounterMetric::Messages, mode.as_str(), saturating_i64(amount)));
        }
    }
    let gauges = [
        (GaugeMetric::SessionsOnline, tick.sessions_online),
        (GaugeMetric::VoiceUsers, tick.voice_users),
        (GaugeMetric::ActiveCalls, tick.active_calls),
    ];
    for (metric, value) in gauges {
        if let Some(value) = value {
            signals.push(UsageSignal::Gauge { metric, dimension: tick.node_id.clone(), value: value as f64 });
        }
    }
    let voice_seconds = tick.voice_users.unwrap_or(0).saturating_mul(u64::from(tick.interval_seconds));
    if voice_seconds > 0 {
        let region = tick.voice_region.as_deref().unwrap_or(UNKNOWN_REGION);
        signals.push(count(CounterMetric::VoiceSeconds, region, saturating_i64(voice_seconds)));
    }
    signals
}

fn saturating_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn event(event_type: &str, actors: &[&str], facts: EventFacts) -> ObservedEvent {
        ObservedEvent {
            id: "evt-1".into(),
            event_type: event_type.into(),
            occurred_at: Utc.with_ymd_and_hms(2026, 10, 16, 12, 0, 0).unwrap(),
            actor_ids: actors.iter().map(|a| a.to_string()).collect(),
            facts,
        }
    }

    fn count(metric: CounterMetric, dimension: &str, amount: i64) -> UsageSignal {
        UsageSignal::Count { metric, dimension: dimension.into(), amount }
    }

    fn gauge(metric: GaugeMetric, dimension: &str, value: f64) -> UsageSignal {
        UsageSignal::Gauge { metric, dimension: dimension.into(), value }
    }

    fn active(user: &str) -> UsageSignal {
        UsageSignal::ActiveUser { user_id: user.into() }
    }

    fn tick(node: &str) -> UsageTick {
        UsageTick { node_id: node.into(), interval_seconds: 10, ..Default::default() }
    }

    #[test]
    fn every_event_is_counted_by_type() {
        let signals = signals_for(&event("GUILD_CREATE", &[], EventFacts::Other));
        assert_eq!(signals, vec![count(CounterMetric::Events, "GUILD_CREATE", 1)]);
    }

    #[test]
    fn actors_become_active_users() {
        let signals = signals_for(&event("TYPING_START", &["42", "43"], EventFacts::Other));
        assert!(signals.contains(&active("42")));
        assert!(signals.contains(&active("43")));
    }

    #[test]
    fn duplicate_actors_are_reported_once() {
        let signals = signals_for(&event("MESSAGE_ACK", &["42", "42"], EventFacts::Other));
        assert_eq!(signals.iter().filter(|s| **s == active("42")).count(), 1);
    }

    #[test]
    fn created_message_counts_as_one_standard_message() {
        let signals = signals_for(&event("MESSAGE_CREATE", &["42"], EventFacts::MessageCreated));
        assert!(signals.contains(&count(CounterMetric::Messages, DeliveryMode::Standard.as_str(), 1)));
        assert!(signals.contains(&count(CounterMetric::Events, "MESSAGE_CREATE", 1)));
    }

    #[test]
    fn usage_tick_reports_private_mode_message_counts() {
        let facts = UsageTick { ephemeral_relayed: 7, p2p_signaled: 3, ..tick("gw-0") };
        let signals = signals_for(&event("USAGE_TICK", &[], EventFacts::UsageTick(facts)));
        assert!(signals.contains(&count(CounterMetric::Messages, "ephemeral", 7)));
        assert!(signals.contains(&count(CounterMetric::Messages, "p2p", 3)));
    }

    #[test]
    fn usage_tick_skips_zero_private_mode_counts() {
        let signals = signals_for(&event("USAGE_TICK", &[], EventFacts::UsageTick(tick("gw-0"))));
        assert!(!signals.iter().any(|s| matches!(s, UsageSignal::Count { metric: CounterMetric::Messages, .. })));
    }

    #[test]
    fn usage_tick_reports_node_gauges() {
        let facts = UsageTick { sessions_online: Some(120), voice_users: Some(8), active_calls: Some(2), ..tick("node-1") };
        let signals = signals_for(&event("USAGE_TICK", &[], EventFacts::UsageTick(facts)));
        assert!(signals.contains(&gauge(GaugeMetric::SessionsOnline, "node-1", 120.0)));
        assert!(signals.contains(&gauge(GaugeMetric::VoiceUsers, "node-1", 8.0)));
        assert!(signals.contains(&gauge(GaugeMetric::ActiveCalls, "node-1", 2.0)));
    }

    #[test]
    fn absent_gauges_are_not_reported() {
        let signals = signals_for(&event("USAGE_TICK", &[], EventFacts::UsageTick(tick("gw-0"))));
        assert!(!signals.iter().any(|s| matches!(s, UsageSignal::Gauge { .. })));
    }

    #[test]
    fn voice_users_accrue_participant_seconds_per_region() {
        let facts = UsageTick { voice_users: Some(8), voice_region: Some("nl-home".into()), ..tick("voice-nl-1") };
        let signals = signals_for(&event("USAGE_TICK", &[], EventFacts::UsageTick(facts)));
        assert!(signals.contains(&count(CounterMetric::VoiceSeconds, "nl-home", 80)));
    }

    #[test]
    fn voice_seconds_fall_back_to_unknown_region() {
        let facts = UsageTick { voice_users: Some(1), ..tick("voice-x") };
        let signals = signals_for(&event("USAGE_TICK", &[], EventFacts::UsageTick(facts)));
        assert!(signals.contains(&count(CounterMetric::VoiceSeconds, UNKNOWN_REGION, 10)));
    }

    #[test]
    fn idle_voice_nodes_accrue_no_voice_seconds() {
        let facts = UsageTick { voice_users: Some(0), ..tick("voice-x") };
        let signals = signals_for(&event("USAGE_TICK", &[], EventFacts::UsageTick(facts)));
        assert!(!signals.iter().any(|s| matches!(s, UsageSignal::Count { metric: CounterMetric::VoiceSeconds, .. })));
    }

    #[test]
    fn voice_quality_is_forwarded_as_a_sample() {
        let sample = VoiceQualitySample { region: "nl-home".into(), jitter_ms: 4.0, loss_pct: 0.5, rtt_ms: 30.0, bitrate_kbps: 64.0 };
        let signals = signals_for(&event("VOICE_QUALITY", &["42"], EventFacts::VoiceQuality(sample.clone())));
        assert!(signals.contains(&UsageSignal::Quality(sample)));
        assert!(signals.contains(&active("42")));
    }
}
