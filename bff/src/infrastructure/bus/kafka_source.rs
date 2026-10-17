use async_trait::async_trait;
use rskafka::client::error::{Error as KafkaError, ProtocolError};
use rskafka::client::partition::{OffsetAt, PartitionClient};

use super::partition_pump::{FetchError, FetchedRecord, RecordSource};

const MAX_FETCH_BYTES: i32 = 4 * 1024 * 1024;
const MAX_WAIT_MS: i32 = 1_000;

/// [`RecordSource`] backed by an rskafka partition client (Redpanda speaks the Kafka API).
pub struct KafkaPartitionSource {
    client: PartitionClient,
}

impl KafkaPartitionSource {
    pub fn new(client: PartitionClient) -> Self {
        Self { client }
    }
}

fn classify(error: KafkaError) -> FetchError {
    match error {
        KafkaError::ServerError { protocol_error: ProtocolError::OffsetOutOfRange, .. } => FetchError::OffsetOutOfRange,
        other => FetchError::Transient(other.to_string()),
    }
}

#[async_trait]
impl RecordSource for KafkaPartitionSource {
    async fn fetch(&self, offset: i64) -> Result<Vec<FetchedRecord>, FetchError> {
        let (records, _high_watermark) = self.client.fetch_records(offset, 1..MAX_FETCH_BYTES, MAX_WAIT_MS).await.map_err(classify)?;
        // Compressed batches can include records before the requested offset.
        Ok(records.into_iter().filter(|r| r.offset >= offset).map(|r| (r.offset, r.record.value)).collect())
    }

    async fn earliest_offset(&self) -> Result<i64, FetchError> {
        self.client.get_offset(OffsetAt::Earliest).await.map_err(classify)
    }
}
