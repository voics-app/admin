use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::application::ports::PortResult;
use crate::application::ports::rollups::{RollupWriter, StreamCheckpoint};
use crate::domain::observed_event::ObservedEvent;
use crate::domain::recent_ids::RecentIds;
use crate::domain::rollup::RollupDelta;
use crate::domain::usage_signal::signals_for;

/// How many recent event ids are remembered for duplicate suppression.
pub const DEDUP_WINDOW: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestOutcome {
    pub counted: usize,
    pub duplicates: usize,
}

/// Folds a batch of bus events into rollups and commits them together with the stream position.
///
/// Ids are only remembered after a successful commit, so a failed batch is fully recounted on retry.
pub struct IngestUsageEvents {
    writer: Arc<dyn RollupWriter>,
    recent: Mutex<RecentIds>,
}

impl IngestUsageEvents {
    pub fn new(writer: Arc<dyn RollupWriter>) -> Self {
        Self { writer, recent: Mutex::new(RecentIds::new(DEDUP_WINDOW)) }
    }

    pub async fn execute(&self, events: &[ObservedEvent], checkpoint: Option<&StreamCheckpoint>) -> PortResult<IngestOutcome> {
        let fresh = self.fresh_events(events);
        let mut delta = RollupDelta::default();
        for event in &fresh {
            for signal in signals_for(event) {
                delta.record(event.occurred_at, &signal);
            }
        }
        if !delta.is_empty() || checkpoint.is_some() {
            self.writer.apply(&delta, checkpoint).await?;
        }
        self.remember(&fresh);
        Ok(IngestOutcome { counted: fresh.len(), duplicates: events.len() - fresh.len() })
    }

    fn fresh_events<'a>(&self, events: &'a [ObservedEvent]) -> Vec<&'a ObservedEvent> {
        let recent = self.recent.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut in_batch = HashSet::new();
        events
            .iter()
            .filter(|e| !recent.contains(&e.id) && in_batch.insert(e.id.as_str()))
            .collect()
    }

    fn remember(&self, events: &[&ObservedEvent]) {
        let mut recent = self.recent.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for event in events {
            recent.first_sighting(&event.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeRollups, fixed_now};
    use crate::domain::observed_event::EventFacts;

    fn message(id: &str) -> ObservedEvent {
        ObservedEvent {
            id: id.into(),
            event_type: "MESSAGE_CREATE".into(),
            occurred_at: fixed_now(),
            actor_ids: vec!["42".into()],
            facts: EventFacts::MessageCreated,
        }
    }

    fn checkpoint(offset: i64) -> StreamCheckpoint {
        StreamCheckpoint { topic: "discordy.message.v1".into(), partition: 0, next_offset: offset }
    }

    fn standard_messages(delta: &RollupDelta) -> i64 {
        delta
            .counters()
            .filter(|(k, _)| k.metric == "messages" && k.granularity == crate::domain::granularity::Granularity::Minute)
            .map(|(_, v)| v)
            .sum()
    }

    #[tokio::test]
    async fn counts_events_and_commits_the_checkpoint_with_them() {
        let store = Arc::new(FakeRollups::default());
        let ingest = IngestUsageEvents::new(store.clone());
        let outcome = ingest.execute(&[message("a"), message("b")], Some(&checkpoint(2))).await.unwrap();
        assert_eq!(outcome, IngestOutcome { counted: 2, duplicates: 0 });
        let applied = store.applied.lock().unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(standard_messages(&applied[0].0), 2);
        assert_eq!(applied[0].1, Some(checkpoint(2)));
    }

    #[tokio::test]
    async fn redelivered_events_are_not_counted_twice() {
        let store = Arc::new(FakeRollups::default());
        let ingest = IngestUsageEvents::new(store.clone());
        ingest.execute(&[message("a")], Some(&checkpoint(1))).await.unwrap();
        let outcome = ingest.execute(&[message("a"), message("b")], Some(&checkpoint(3))).await.unwrap();
        assert_eq!(outcome, IngestOutcome { counted: 1, duplicates: 1 });
        assert_eq!(standard_messages(&store.applied.lock().unwrap()[1].0), 1);
    }

    #[tokio::test]
    async fn duplicates_within_one_batch_are_counted_once() {
        let store = Arc::new(FakeRollups::default());
        let ingest = IngestUsageEvents::new(store.clone());
        let outcome = ingest.execute(&[message("a"), message("a")], None).await.unwrap();
        assert_eq!(outcome.duplicates, 1);
        assert_eq!(standard_messages(&store.applied.lock().unwrap()[0].0), 1);
    }

    #[tokio::test]
    async fn a_failed_commit_leaves_events_countable_on_retry() {
        let store = Arc::new(FakeRollups::default());
        let ingest = IngestUsageEvents::new(store.clone());
        *store.fail_writes.lock().unwrap() = true;
        assert!(ingest.execute(&[message("a")], None).await.is_err());
        *store.fail_writes.lock().unwrap() = false;
        let outcome = ingest.execute(&[message("a")], None).await.unwrap();
        assert_eq!(outcome.counted, 1);
    }

    #[tokio::test]
    async fn an_all_duplicate_batch_still_advances_the_checkpoint() {
        let store = Arc::new(FakeRollups::default());
        let ingest = IngestUsageEvents::new(store.clone());
        ingest.execute(&[message("a")], Some(&checkpoint(1))).await.unwrap();
        ingest.execute(&[message("a")], Some(&checkpoint(2))).await.unwrap();
        let applied = store.applied.lock().unwrap();
        assert!(applied[1].0.is_empty());
        assert_eq!(applied[1].1, Some(checkpoint(2)));
    }

    #[tokio::test]
    async fn an_empty_batch_without_checkpoint_writes_nothing() {
        let store = Arc::new(FakeRollups::default());
        let ingest = IngestUsageEvents::new(store.clone());
        ingest.execute(&[], None).await.unwrap();
        assert!(store.applied.lock().unwrap().is_empty());
    }
}
