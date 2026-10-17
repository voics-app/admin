use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use super::envelope_decoder::decode_envelope;
use crate::application::ports::rollups::StreamCheckpoint;
use crate::application::use_cases::ingest_usage_events::IngestUsageEvents;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FetchError {
    /// The requested offset was pruned by retention (or never existed).
    #[error("offset out of range")]
    OffsetOutOfRange,
    #[error("fetch failed: {0}")]
    Transient(String),
}

/// One fetched record: its offset and raw value.
pub type FetchedRecord = (i64, Option<Vec<u8>>);

/// Reads raw records from one topic partition.
#[async_trait]
pub trait RecordSource: Send + Sync {
    async fn fetch(&self, offset: i64) -> Result<Vec<FetchedRecord>, FetchError>;
    async fn earliest_offset(&self) -> Result<i64, FetchError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PumpReport {
    pub fetched: usize,
    pub counted: usize,
    pub duplicates: usize,
    pub undecodable: usize,
}

#[derive(Debug, Error)]
pub enum PumpError {
    #[error(transparent)]
    Fetch(#[from] FetchError),
    #[error("rollup commit failed: {0}")]
    Commit(String),
}

/// Moves one partition forward: fetch, decode, ingest, and advance only after a successful commit.
pub struct PartitionPump {
    topic: String,
    partition: i32,
    next_offset: i64,
    source: Arc<dyn RecordSource>,
    ingest: Arc<IngestUsageEvents>,
}

impl PartitionPump {
    pub fn new(topic: &str, partition: i32, next_offset: i64, source: Arc<dyn RecordSource>, ingest: Arc<IngestUsageEvents>) -> Self {
        Self { topic: topic.to_owned(), partition, next_offset, source, ingest }
    }

    pub fn next_offset(&self) -> i64 {
        self.next_offset
    }

    pub async fn step(&mut self) -> Result<PumpReport, PumpError> {
        let records = match self.source.fetch(self.next_offset).await {
            Ok(records) => records,
            Err(FetchError::OffsetOutOfRange) => {
                self.next_offset = self.source.earliest_offset().await?;
                return Ok(PumpReport::default());
            }
            Err(error) => return Err(error.into()),
        };
        let Some(last_offset) = records.iter().map(|(offset, _)| *offset).max() else {
            return Ok(PumpReport::default());
        };

        let mut undecodable = 0;
        let events: Vec<_> = records
            .iter()
            .filter_map(|(_, value)| match value.as_deref().map(decode_envelope) {
                Some(Ok(event)) => Some(event),
                Some(Err(error)) => {
                    tracing::warn!(topic = %self.topic, partition = self.partition, %error, "skipping undecodable event");
                    undecodable += 1;
                    None
                }
                None => {
                    undecodable += 1;
                    None
                }
            })
            .collect();

        let checkpoint = StreamCheckpoint { topic: self.topic.clone(), partition: self.partition, next_offset: last_offset + 1 };
        let outcome = self
            .ingest
            .execute(&events, Some(&checkpoint))
            .await
            .map_err(|e| PumpError::Commit(e.to_string()))?;
        self.next_offset = checkpoint.next_offset;
        Ok(PumpReport { fetched: records.len(), counted: outcome.counted, duplicates: outcome.duplicates, undecodable })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::FakeRollups;
    use std::sync::Mutex;

    struct ScriptedSource {
        responses: Mutex<Vec<Result<Vec<FetchedRecord>, FetchError>>>,
        requested: Mutex<Vec<i64>>,
        earliest: i64,
    }

    impl ScriptedSource {
        fn new(responses: Vec<Result<Vec<FetchedRecord>, FetchError>>) -> Arc<Self> {
            Arc::new(Self { responses: Mutex::new(responses), requested: Mutex::new(vec![]), earliest: 500 })
        }
    }

    #[async_trait]
    impl RecordSource for ScriptedSource {
        async fn fetch(&self, offset: i64) -> Result<Vec<FetchedRecord>, FetchError> {
            self.requested.lock().unwrap().push(offset);
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() { Ok(vec![]) } else { responses.remove(0) }
        }

        async fn earliest_offset(&self) -> Result<i64, FetchError> {
            Ok(self.earliest)
        }
    }

    fn record(offset: i64, id: &str) -> FetchedRecord {
        let json = format!(r#"{{"id":"{id}","type":"MESSAGE_CREATE","v":1,"occurred_at":"2026-10-16T12:00:00Z","data":{{"author":{{"id":"42"}}}}}}"#);
        (offset, Some(json.into_bytes()))
    }

    fn pump(source: Arc<ScriptedSource>, store: Arc<FakeRollups>) -> PartitionPump {
        PartitionPump::new("discordy.message.v1", 3, 10, source, Arc::new(IngestUsageEvents::new(store)))
    }

    #[tokio::test]
    async fn commits_a_batch_with_the_offset_after_its_last_record() {
        let store = Arc::new(FakeRollups::default());
        let mut pump = pump(ScriptedSource::new(vec![Ok(vec![record(10, "a"), record(11, "b")])]), store.clone());
        let report = pump.step().await.unwrap();
        assert_eq!(report, PumpReport { fetched: 2, counted: 2, duplicates: 0, undecodable: 0 });
        assert_eq!(pump.next_offset(), 12);
        let checkpoint = store.applied.lock().unwrap()[0].1.clone().unwrap();
        assert_eq!((checkpoint.topic.as_str(), checkpoint.partition, checkpoint.next_offset), ("discordy.message.v1", 3, 12));
    }

    #[tokio::test]
    async fn undecodable_records_are_skipped_but_their_offsets_are_consumed() {
        let store = Arc::new(FakeRollups::default());
        let source = ScriptedSource::new(vec![Ok(vec![(10, Some(b"garbage".to_vec())), (11, None), record(12, "c")])]);
        let mut pump = pump(source, store);
        let report = pump.step().await.unwrap();
        assert_eq!((report.counted, report.undecodable), (1, 2));
        assert_eq!(pump.next_offset(), 13);
    }

    #[tokio::test]
    async fn an_empty_fetch_commits_nothing() {
        let store = Arc::new(FakeRollups::default());
        let mut pump = pump(ScriptedSource::new(vec![Ok(vec![])]), store.clone());
        assert_eq!(pump.step().await.unwrap(), PumpReport::default());
        assert!(store.applied.lock().unwrap().is_empty());
        assert_eq!(pump.next_offset(), 10);
    }

    #[tokio::test]
    async fn a_pruned_offset_restarts_from_the_earliest_retained_record() {
        let store = Arc::new(FakeRollups::default());
        let source = ScriptedSource::new(vec![Err(FetchError::OffsetOutOfRange), Ok(vec![record(500, "a")])]);
        let mut pump = pump(source.clone(), store);
        pump.step().await.unwrap();
        pump.step().await.unwrap();
        assert_eq!(*source.requested.lock().unwrap(), vec![10, 500]);
    }

    #[tokio::test]
    async fn a_failed_commit_keeps_the_offset_so_the_batch_is_refetched() {
        let store = Arc::new(FakeRollups::default());
        *store.fail_writes.lock().unwrap() = true;
        let mut pump = pump(ScriptedSource::new(vec![Ok(vec![record(10, "a")])]), store);
        assert!(matches!(pump.step().await, Err(PumpError::Commit(_))));
        assert_eq!(pump.next_offset(), 10);
    }
}
