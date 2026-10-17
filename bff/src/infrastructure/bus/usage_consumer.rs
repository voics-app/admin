use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use rskafka::client::partition::UnknownTopicHandling;
use rskafka::client::{Client, ClientBuilder};
use tokio::sync::watch;
use tokio::task::JoinSet;

use super::kafka_source::KafkaPartitionSource;
use super::partition_pump::{PartitionPump, RecordSource};
use crate::application::ports::rollups::RollupWriter;
use crate::application::use_cases::ingest_usage_events::IngestUsageEvents;
use crate::infrastructure::contracts::TOPICS;

const RETRY_DELAY: Duration = Duration::from_secs(5);
const TOPIC_RESCAN_INTERVAL: Duration = Duration::from_secs(60);

/// Consumes every P6 topic, one task per partition, committing offsets in Postgres alongside rollups.
///
/// Offsets live in `discordy_admin.consumer_offsets` rather than in a Kafka consumer group so a
/// batch's rollups and its position commit atomically (effectively-once counting).
pub struct UsageConsumer {
    brokers: Vec<String>,
    ingest: Arc<IngestUsageEvents>,
    writer: Arc<dyn RollupWriter>,
}

impl UsageConsumer {
    pub fn new(
        brokers: Vec<String>,
        ingest: Arc<IngestUsageEvents>,
        writer: Arc<dyn RollupWriter>,
    ) -> Self {
        Self {
            brokers,
            ingest,
            writer,
        }
    }

    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let Some(client) = self.connect(&mut shutdown).await else {
            return;
        };
        let client = Arc::new(client);
        let mut running: BTreeSet<(String, i32)> = BTreeSet::new();
        let mut tasks = JoinSet::new();
        loop {
            match client.list_topics().await {
                Ok(topics) => {
                    for topic in topics
                        .into_iter()
                        .filter(|t| TOPICS.contains(&t.name.as_str()))
                    {
                        for partition in topic.partitions {
                            if running.insert((topic.name.clone(), partition)) {
                                tasks.spawn(self.consume(
                                    client.clone(),
                                    topic.name.clone(),
                                    partition,
                                    shutdown.clone(),
                                ));
                            }
                        }
                    }
                }
                Err(error) => tracing::warn!(%error, "listing topics failed"),
            }
            tokio::select! {
                _ = shutdown.changed() => break,
                _ = tokio::time::sleep(TOPIC_RESCAN_INTERVAL) => {}
            }
        }
        while tasks.join_next().await.is_some() {}
    }

    async fn connect(&self, shutdown: &mut watch::Receiver<bool>) -> Option<Client> {
        loop {
            match ClientBuilder::new(self.brokers.clone())
                .client_id("discordy-admin-bff")
                .build()
                .await
            {
                Ok(client) => {
                    tracing::info!(brokers = ?self.brokers, "connected to event bus");
                    return Some(client);
                }
                Err(error) => tracing::warn!(%error, "event bus unreachable, retrying"),
            }
            tokio::select! {
                _ = shutdown.changed() => return None,
                _ = tokio::time::sleep(RETRY_DELAY) => {}
            }
        }
    }

    fn consume(
        &self,
        client: Arc<Client>,
        topic: String,
        partition: i32,
        mut shutdown: watch::Receiver<bool>,
    ) -> impl Future<Output = ()> + use<> {
        let ingest = self.ingest.clone();
        let writer = self.writer.clone();
        async move {
            let Some(mut pump) =
                open_pump(&client, &topic, partition, writer, ingest, &mut shutdown).await
            else {
                return;
            };
            tracing::info!(%topic, partition, offset = pump.next_offset(), "consuming partition");
            loop {
                if *shutdown.borrow() {
                    return;
                }
                match pump.step().await {
                    Ok(report) => {
                        metrics::counter!("admin_bff_events_counted_total", "topic" => topic.clone()).increment(report.counted as u64);
                        metrics::counter!("admin_bff_events_duplicate_total", "topic" => topic.clone()).increment(report.duplicates as u64);
                        metrics::counter!("admin_bff_events_undecodable_total", "topic" => topic.clone()).increment(report.undecodable as u64);
                    }
                    Err(error) => {
                        tracing::warn!(%topic, partition, %error, "partition step failed, retrying");
                        tokio::select! {
                            _ = shutdown.changed() => return,
                            _ = tokio::time::sleep(RETRY_DELAY) => {}
                        }
                    }
                }
            }
        }
    }
}

async fn open_pump(
    client: &Client,
    topic: &str,
    partition: i32,
    writer: Arc<dyn RollupWriter>,
    ingest: Arc<IngestUsageEvents>,
    shutdown: &mut watch::Receiver<bool>,
) -> Option<PartitionPump> {
    loop {
        let attempt = async {
            let partition_client = client
                .partition_client(topic, partition, UnknownTopicHandling::Retry)
                .await
                .map_err(|e| e.to_string())?;
            let source: Arc<dyn RecordSource> =
                Arc::new(KafkaPartitionSource::new(partition_client));
            let offset = match writer
                .checkpoint(topic, partition)
                .await
                .map_err(|e| e.to_string())?
            {
                Some(offset) => offset,
                None => source.earliest_offset().await.map_err(|e| e.to_string())?,
            };
            Ok::<_, String>(PartitionPump::new(
                topic,
                partition,
                offset,
                source,
                ingest.clone(),
            ))
        };
        match attempt.await {
            Ok(pump) => return Some(pump),
            Err(error) => {
                tracing::warn!(%topic, partition, %error, "opening partition failed, retrying")
            }
        }
        tokio::select! {
            _ = shutdown.changed() => return None,
            _ = tokio::time::sleep(RETRY_DELAY) => {}
        }
    }
}
