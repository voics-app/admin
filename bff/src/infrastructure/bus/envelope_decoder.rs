use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::domain::observed_event::{EventFacts, ObservedEvent, UsageTick, VoiceQualitySample};
use crate::domain::usage_signal::UNKNOWN_REGION;
use crate::infrastructure::contracts::{EnvelopeHead, EventData};

/// USAGE_TICK interval assumed when a producer omits it (seconds).
const DEFAULT_TICK_INTERVAL_S: u32 = 60;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("not a P6 envelope: {0}")]
    Malformed(String),
    #[error("occurred_at is not RFC 3339: {0}")]
    BadTimestamp(String),
}

/// Turns a raw P6 envelope into the content-free [`ObservedEvent`] the aggregator counts.
pub fn decode_envelope(payload: &[u8]) -> Result<ObservedEvent, DecodeError> {
    let head: EnvelopeHead = serde_json::from_slice(payload).map_err(|e| DecodeError::Malformed(e.to_string()))?;
    let occurred_at = DateTime::parse_from_rfc3339(&head.occurred_at)
        .map_err(|_| DecodeError::BadTimestamp(head.occurred_at.clone()))?
        .with_timezone(&Utc);
    // An unexpected payload shape only loses the type-specific facts, never the event itself.
    let data: EventData = head
        .data
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw.get()).ok())
        .unwrap_or_default();

    let actor_ids = actors(&head, &data);
    let facts = facts(&head.event_type, data);
    Ok(ObservedEvent { id: head.id, event_type: head.event_type, occurred_at, actor_ids, facts })
}

fn actors(head: &EnvelopeHead, data: &EventData) -> Vec<String> {
    let human = |r: &crate::infrastructure::contracts::AuthorRef| (!r.bot).then(|| r.id.clone());
    match head.event_type.as_str() {
        "MESSAGE_CREATE" | "MESSAGE_UPDATE" => data.author.as_ref().and_then(human).into_iter().collect(),
        "TYPING_START" | "MESSAGE_REACTION_ADD" | "MESSAGE_REACTION_REMOVE" | "MESSAGE_ACK" | "VOICE_QUALITY" => {
            data.user_id.clone().into_iter().collect()
        }
        "GUILD_MEMBER_ADD" => data.user.as_ref().and_then(human).into_iter().collect(),
        "INTERACTION_CREATE" => data
            .member
            .as_ref()
            .and_then(|m| m.user.as_ref())
            .or(data.user.as_ref())
            .and_then(human)
            .into_iter()
            .collect(),
        "USER_UPDATE" | "USER_SETTINGS_UPDATE" => head.target_user_ids.clone().unwrap_or_default(),
        "USAGE_TICK" => data.active_user_ids.clone().unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn facts(event_type: &str, data: EventData) -> EventFacts {
    match event_type {
        "MESSAGE_CREATE" => EventFacts::MessageCreated,
        "USAGE_TICK" => EventFacts::UsageTick(UsageTick {
            node_id: data.node_id.unwrap_or_default(),
            interval_seconds: data.interval_s.unwrap_or(DEFAULT_TICK_INTERVAL_S),
            sessions_online: data.sessions_online,
            ephemeral_relayed: data.ephemeral_relayed.unwrap_or(0),
            p2p_signaled: data.p2p_signaled.unwrap_or(0),
            voice_users: data.voice_users,
            active_calls: data.active_calls,
            voice_region: data.region,
        }),
        "VOICE_QUALITY" => EventFacts::VoiceQuality(VoiceQualitySample {
            region: data.region.unwrap_or_else(|| UNKNOWN_REGION.to_owned()),
            jitter_ms: data.jitter_ms.unwrap_or(0.0),
            loss_pct: data.loss_pct.unwrap_or(0.0),
            rtt_ms: data.rtt_ms.unwrap_or(0.0),
            bitrate_kbps: data.bitrate_kbps.unwrap_or(0.0),
        }),
        _ => EventFacts::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn envelope(event_type: &str, data: serde_json::Value) -> Vec<u8> {
        json!({
            "id": "0192f0a0-0000-7000-8000-000000000001",
            "type": event_type,
            "v": 1,
            "occurred_at": "2026-10-16T12:00:05.250Z",
            "guild_id": "100",
            "channel_id": "200",
            "target_user_ids": null,
            "data": data
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn decodes_the_envelope_head() {
        let event = decode_envelope(&envelope("GUILD_CREATE", json!({"id": "100", "name": "Lounge"}))).unwrap();
        assert_eq!(event.id, "0192f0a0-0000-7000-8000-000000000001");
        assert_eq!(event.event_type, "GUILD_CREATE");
        assert_eq!(event.occurred_at.to_rfc3339(), "2026-10-16T12:00:05.250+00:00");
        assert_eq!(event.facts, EventFacts::Other);
    }

    #[test]
    fn a_created_message_yields_its_author_and_never_its_content() {
        let payload = envelope(
            "MESSAGE_CREATE",
            json!({"id": "1", "content": "top secret plans", "author": {"id": "42", "username": "alice"}, "embeds": []}),
        );
        let event = decode_envelope(&payload).unwrap();
        assert_eq!(event.facts, EventFacts::MessageCreated);
        assert_eq!(event.actor_ids, vec!["42"]);
        assert!(!format!("{event:?}").contains("top secret"));
    }

    #[test]
    fn bot_authors_are_not_active_users() {
        let event = decode_envelope(&envelope("MESSAGE_CREATE", json!({"author": {"id": "7", "bot": true}}))).unwrap();
        assert!(event.actor_ids.is_empty());
    }

    #[test]
    fn typing_and_reactions_identify_the_acting_user() {
        for kind in ["TYPING_START", "MESSAGE_REACTION_ADD", "MESSAGE_ACK"] {
            let event = decode_envelope(&envelope(kind, json!({"user_id": "42"}))).unwrap();
            assert_eq!(event.actor_ids, vec!["42"], "{kind}");
        }
    }

    #[test]
    fn interactions_identify_the_invoking_member() {
        let event = decode_envelope(&envelope("INTERACTION_CREATE", json!({"member": {"user": {"id": "42"}}}))).unwrap();
        assert_eq!(event.actor_ids, vec!["42"]);
    }

    #[test]
    fn self_updates_use_the_target_users() {
        let mut value: serde_json::Value = serde_json::from_slice(&envelope("USER_SETTINGS_UPDATE", json!({}))).unwrap();
        value["target_user_ids"] = json!(["42"]);
        let event = decode_envelope(value.to_string().as_bytes()).unwrap();
        assert_eq!(event.actor_ids, vec!["42"]);
    }

    #[test]
    fn usage_ticks_carry_node_counters() {
        let payload = envelope(
            "USAGE_TICK",
            json!({"node_id": "gw-0", "interval_s": 10, "sessions_online": 120, "ephemeral_relayed": 4, "p2p_signaled": 1, "active_user_ids": ["1", "2"]}),
        );
        let event = decode_envelope(&payload).unwrap();
        let EventFacts::UsageTick(tick) = event.facts else { panic!("expected a usage tick") };
        assert_eq!(tick.node_id, "gw-0");
        assert_eq!(tick.interval_seconds, 10);
        assert_eq!(tick.sessions_online, Some(120));
        assert_eq!((tick.ephemeral_relayed, tick.p2p_signaled), (4, 1));
        assert_eq!(event.actor_ids, vec!["1", "2"]);
    }

    #[test]
    fn voice_quality_defaults_to_the_unknown_region() {
        let payload = envelope("VOICE_QUALITY", json!({"channel_id": "9", "user_id": "42", "jitter_ms": 3.5, "loss_pct": 0.2, "rtt_ms": 41.0, "bitrate_kbps": 64.0}));
        let event = decode_envelope(&payload).unwrap();
        let EventFacts::VoiceQuality(sample) = event.facts else { panic!("expected voice quality") };
        assert_eq!(sample.region, UNKNOWN_REGION);
        assert_eq!(sample.jitter_ms, 3.5);
        assert_eq!(event.actor_ids, vec!["42"]);
    }

    #[test]
    fn an_unexpected_payload_shape_still_counts_the_event() {
        let event = decode_envelope(&envelope("MESSAGE_CREATE", json!({"author": "not-an-object"}))).unwrap();
        assert_eq!(event.facts, EventFacts::MessageCreated);
        assert!(event.actor_ids.is_empty());
    }

    #[test]
    fn rejects_non_envelopes_and_bad_timestamps() {
        assert!(matches!(decode_envelope(b"not json"), Err(DecodeError::Malformed(_))));
        let bad_time = json!({"id": "x", "type": "GUILD_CREATE", "occurred_at": "yesterday"}).to_string();
        assert!(matches!(decode_envelope(bad_time.as_bytes()), Err(DecodeError::BadTimestamp(_))));
    }
}
