use async_trait::async_trait;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use sqlx::{PgPool, Postgres, Transaction};

use crate::application::ports::rollups::{
    ActiveUsersPoint, DimensionPoint, RollupReader, RollupWriter, StreamCheckpoint,
};
use crate::application::ports::{PortError, PortResult};
use crate::domain::granularity::Granularity;
use crate::domain::metric::{CounterMetric, GaugeMetric};
use crate::domain::rollup::{QualityTotals, RollupDelta, SeriesKey};

/// Voice-quality sums are stored as additive counters with the region as dimension.
const QUALITY_SAMPLES: &str = "voice_quality.samples";
const QUALITY_JITTER: &str = "voice_quality.jitter_ms_sum";
const QUALITY_LOSS: &str = "voice_quality.loss_pct_sum";
const QUALITY_RTT: &str = "voice_quality.rtt_ms_sum";
const QUALITY_BITRATE: &str = "voice_quality.bitrate_kbps_sum";

/// How long each rollup resolution is kept.
pub fn retention(granularity: Granularity) -> Option<Duration> {
    match granularity {
        Granularity::Minute => Some(Duration::days(7)),
        Granularity::Hour => Some(Duration::days(90)),
        Granularity::Day => None,
    }
}

/// Active-user rows are kept long enough for a year of MAU history.
const ACTIVE_USER_RETENTION_DAYS: i64 = 400;

pub(super) fn db_error(error: sqlx::Error) -> PortError {
    PortError::Unavailable(format!("postgres: {error}"))
}

fn table(granularity: Granularity) -> &'static str {
    match granularity {
        Granularity::Minute => "usage_rollup_minute",
        Granularity::Hour => "usage_rollup_hour",
        Granularity::Day => "usage_rollup_day",
    }
}

#[derive(Default)]
struct Columns {
    metrics: Vec<String>,
    dimensions: Vec<String>,
    buckets: Vec<DateTime<Utc>>,
    values: Vec<f64>,
}

impl Columns {
    fn push(&mut self, metric: &str, dimension: &str, bucket: DateTime<Utc>, value: f64) {
        self.metrics.push(metric.to_owned());
        self.dimensions.push(dimension.to_owned());
        self.buckets.push(bucket);
        self.values.push(value);
    }
}

#[derive(Clone, Copy)]
enum Merge {
    Sum,
    Peak,
}

/// Postgres adapter for usage rollups and aggregator checkpoints (`discordy_admin`).
#[derive(Clone)]
pub struct PgRollupStore {
    pool: PgPool,
}

impl PgRollupStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Deletes rollups older than each resolution's retention.
    pub async fn prune(&self, now: DateTime<Utc>) -> PortResult<()> {
        for granularity in Granularity::ALL {
            if let Some(keep) = retention(granularity) {
                let sql = format!("DELETE FROM {} WHERE bucket < $1", table(granularity));
                sqlx::query(&sql)
                    .bind(now - keep)
                    .execute(&self.pool)
                    .await
                    .map_err(db_error)?;
            }
        }
        sqlx::query("DELETE FROM active_users_daily WHERE day < $1")
            .bind(now.date_naive() - Duration::days(ACTIVE_USER_RETENTION_DAYS))
            .execute(&self.pool)
            .await
            .map_err(db_error)?;
        Ok(())
    }

    async fn upsert(
        tx: &mut Transaction<'_, Postgres>,
        granularity: Granularity,
        columns: &Columns,
        merge: Merge,
    ) -> Result<(), sqlx::Error> {
        if columns.metrics.is_empty() {
            return Ok(());
        }
        let name = table(granularity);
        let merged = match merge {
            Merge::Sum => format!("{name}.value + EXCLUDED.value"),
            Merge::Peak => format!("GREATEST({name}.value, EXCLUDED.value)"),
        };
        let sql = format!(
            "INSERT INTO {name} (metric, dimension, bucket, value) \
             SELECT * FROM UNNEST($1::text[], $2::text[], $3::timestamptz[], $4::float8[]) \
             ON CONFLICT (metric, bucket, dimension) DO UPDATE SET value = {merged}"
        );
        sqlx::query(&sql)
            .bind(&columns.metrics)
            .bind(&columns.dimensions)
            .bind(&columns.buckets)
            .bind(&columns.values)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    async fn series(
        &self,
        metric: &str,
        granularity: Granularity,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> PortResult<Vec<DimensionPoint>> {
        let sql = format!(
            "SELECT bucket, dimension, value FROM {} WHERE metric = $1 AND bucket >= $2 AND bucket < $3 ORDER BY bucket, dimension",
            table(granularity)
        );
        let rows: Vec<(DateTime<Utc>, String, f64)> = sqlx::query_as(&sql)
            .bind(metric)
            .bind(from)
            .bind(to)
            .fetch_all(&self.pool)
            .await
            .map_err(db_error)?;
        Ok(rows
            .into_iter()
            .map(|(bucket, dimension, value)| DimensionPoint {
                bucket,
                dimension,
                value,
            })
            .collect())
    }
}

fn split_by_granularity<'a>(cells: impl Iterator<Item = (&'a SeriesKey, f64)>) -> [Columns; 3] {
    let mut columns: [Columns; 3] = Default::default();
    for (key, value) in cells {
        columns[granularity_index(key.granularity)].push(
            key.metric,
            &key.dimension,
            key.bucket,
            value,
        );
    }
    columns
}

fn granularity_index(granularity: Granularity) -> usize {
    match granularity {
        Granularity::Minute => 0,
        Granularity::Hour => 1,
        Granularity::Day => 2,
    }
}

#[async_trait]
impl RollupWriter for PgRollupStore {
    async fn apply(
        &self,
        delta: &RollupDelta,
        checkpoint: Option<&StreamCheckpoint>,
    ) -> PortResult<()> {
        let counters = split_by_granularity(delta.counters().map(|(k, v)| (k, v as f64)));
        let gauges = split_by_granularity(delta.gauges());
        let mut quality: [Columns; 3] = Default::default();
        for (key, totals) in delta.quality() {
            let cols = &mut quality[granularity_index(key.granularity)];
            cols.push(
                QUALITY_SAMPLES,
                &key.region,
                key.bucket,
                totals.samples as f64,
            );
            cols.push(QUALITY_JITTER, &key.region, key.bucket, totals.jitter_ms);
            cols.push(QUALITY_LOSS, &key.region, key.bucket, totals.loss_pct);
            cols.push(QUALITY_RTT, &key.region, key.bucket, totals.rtt_ms);
            cols.push(
                QUALITY_BITRATE,
                &key.region,
                key.bucket,
                totals.bitrate_kbps,
            );
        }

        let mut tx = self.pool.begin().await.map_err(db_error)?;
        for granularity in Granularity::ALL {
            let i = granularity_index(granularity);
            Self::upsert(&mut tx, granularity, &counters[i], Merge::Sum)
                .await
                .map_err(db_error)?;
            Self::upsert(&mut tx, granularity, &quality[i], Merge::Sum)
                .await
                .map_err(db_error)?;
            Self::upsert(&mut tx, granularity, &gauges[i], Merge::Peak)
                .await
                .map_err(db_error)?;
        }

        let (days, users): (Vec<NaiveDate>, Vec<String>) = delta.active_users().cloned().unzip();
        if !days.is_empty() {
            sqlx::query("INSERT INTO active_users_daily (day, user_id) SELECT * FROM UNNEST($1::date[], $2::text[]) ON CONFLICT DO NOTHING")
                .bind(&days)
                .bind(&users)
                .execute(&mut *tx)
                .await
                .map_err(db_error)?;
        }

        if let Some(checkpoint) = checkpoint {
            sqlx::query(
                "INSERT INTO consumer_offsets (topic, partition, next_offset, updated_at) VALUES ($1, $2, $3, now()) \
                 ON CONFLICT (topic, partition) DO UPDATE SET next_offset = EXCLUDED.next_offset, updated_at = now()",
            )
            .bind(&checkpoint.topic)
            .bind(checkpoint.partition)
            .bind(checkpoint.next_offset)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        }
        tx.commit().await.map_err(db_error)
    }

    async fn checkpoint(&self, topic: &str, partition: i32) -> PortResult<Option<i64>> {
        sqlx::query_scalar(
            "SELECT next_offset FROM consumer_offsets WHERE topic = $1 AND partition = $2",
        )
        .bind(topic)
        .bind(partition)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_error)
    }
}

#[async_trait]
impl RollupReader for PgRollupStore {
    async fn counters(
        &self,
        metric: CounterMetric,
        granularity: Granularity,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> PortResult<Vec<DimensionPoint>> {
        self.series(metric.as_str(), granularity, from, to).await
    }

    async fn gauges(
        &self,
        metric: GaugeMetric,
        granularity: Granularity,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> PortResult<Vec<DimensionPoint>> {
        self.series(metric.as_str(), granularity, from, to).await
    }

    async fn active_users(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> PortResult<Vec<ActiveUsersPoint>> {
        let rows: Vec<(NaiveDate, i64, i64)> = sqlx::query_as(
            "SELECT d::date, \
                (SELECT count(*) FROM active_users_daily a WHERE a.day = d::date), \
                (SELECT count(DISTINCT user_id) FROM active_users_daily a WHERE a.day > d::date - 30 AND a.day <= d::date) \
             FROM generate_series($1::date, $2::date, interval '1 day') AS d ORDER BY 1",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)?;
        Ok(rows
            .into_iter()
            .map(|(day, dau, mau)| ActiveUsersPoint {
                day,
                dau: dau.max(0) as u64,
                mau: mau.max(0) as u64,
            })
            .collect())
    }

    async fn quality_by_region(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> PortResult<Vec<(String, QualityTotals)>> {
        let rows: Vec<(String, String, f64)> = sqlx::query_as(
            "SELECT dimension, metric, sum(value) FROM usage_rollup_minute \
             WHERE metric LIKE 'voice_quality.%' AND bucket >= $1 AND bucket < $2 GROUP BY dimension, metric ORDER BY dimension",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)?;

        let mut regions: Vec<(String, QualityTotals)> = Vec::new();
        for (region, metric, value) in rows {
            if regions.last().is_none_or(|(r, _)| *r != region) {
                regions.push((region, QualityTotals::default()));
            }
            if let Some((_, totals)) = regions.last_mut() {
                match metric.as_str() {
                    QUALITY_SAMPLES => totals.samples = value as i64,
                    QUALITY_JITTER => totals.jitter_ms = value,
                    QUALITY_LOSS => totals.loss_pct = value,
                    QUALITY_RTT => totals.rtt_ms = value,
                    QUALITY_BITRATE => totals.bitrate_kbps = value,
                    _ => {}
                }
            }
        }
        Ok(regions)
    }

    async fn is_reachable(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }
}
