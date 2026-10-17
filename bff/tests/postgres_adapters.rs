//! Integration tests for the Postgres adapters against a real Postgres 17 (testcontainers).

use admin_bff::application::ports::PortError;
use admin_bff::application::ports::audit_log::AuditLog;
use admin_bff::application::ports::feature_flags::FeatureFlagRepository;
use admin_bff::application::ports::rollups::{RollupReader, RollupWriter, StreamCheckpoint};
use admin_bff::domain::audit::{AuditAction, NewAuditEntry};
use admin_bff::domain::feature_flag::{FeatureFlag, FlagKey, RolloutPercent};
use admin_bff::domain::granularity::Granularity;
use admin_bff::domain::metric::{CounterMetric, GaugeMetric};
use admin_bff::domain::observed_event::VoiceQualitySample;
use admin_bff::domain::rollup::RollupDelta;
use admin_bff::domain::usage_signal::UsageSignal;
use admin_bff::infrastructure::postgres::audit_log::PgAuditLog;
use admin_bff::infrastructure::postgres::connect_and_migrate;
use admin_bff::infrastructure::postgres::feature_flags::PgFeatureFlags;
use admin_bff::infrastructure::postgres::rollup_store::PgRollupStore;
use chrono::{DateTime, Duration, TimeZone, Utc};
use sqlx::PgPool;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

async fn database() -> (ContainerAsync<Postgres>, PgPool) {
    let name = format!(
        "admin-pg-test-{}",
        std::process::id() as u64 * 1000 + rand_suffix()
    );
    let container = Postgres::default()
        .with_tag("17-alpine")
        .with_container_name(name)
        .start()
        .await
        .expect("postgres container");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped port");
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = connect_and_migrate(&url).await.expect("migrations");
    (container, pool)
}

fn rand_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    u64::from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
            % 1000,
    )
}

fn at(h: u32, m: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 16, h, m, 0).unwrap()
}

fn messages(amount: i64) -> UsageSignal {
    UsageSignal::Count {
        metric: CounterMetric::Messages,
        dimension: "standard".into(),
        amount,
    }
}

fn checkpoint(offset: i64) -> StreamCheckpoint {
    StreamCheckpoint {
        topic: "discordy.message.v1".into(),
        partition: 0,
        next_offset: offset,
    }
}

#[tokio::test]
async fn postgres_adapters_round_trip() {
    let (_container, pool) = database().await;
    rollups_sum_counters_and_keep_gauge_peaks_across_batches(&pool).await;
    checkpoints_commit_with_rollups(&pool).await;
    active_users_yield_dau_and_rolling_mau(&pool).await;
    voice_quality_sums_are_read_back_per_region(&pool).await;
    pruning_drops_expired_minute_buckets(&pool).await;
    feature_flags_upsert_list_and_delete(&pool).await;
    audit_log_pages_newest_first(&pool).await;
}

async fn rollups_sum_counters_and_keep_gauge_peaks_across_batches(pool: &PgPool) {
    let store = PgRollupStore::new(pool.clone());
    let sessions = |v: f64| UsageSignal::Gauge {
        metric: GaugeMetric::SessionsOnline,
        dimension: "gw-0".into(),
        value: v,
    };
    let mut first = RollupDelta::default();
    first.record(at(10, 1), &messages(3));
    first.record(at(10, 1), &sessions(50.0));
    store.apply(&first, None).await.unwrap();
    let mut second = RollupDelta::default();
    second.record(at(10, 1), &messages(4));
    second.record(at(10, 2), &messages(1));
    second.record(at(10, 1), &sessions(20.0));
    store.apply(&second, None).await.unwrap();

    let minutes = store
        .counters(
            CounterMetric::Messages,
            Granularity::Minute,
            at(10, 0),
            at(11, 0),
        )
        .await
        .unwrap();
    assert_eq!(
        minutes.iter().map(|p| p.value).collect::<Vec<_>>(),
        vec![7.0, 1.0]
    );
    let hours = store
        .counters(
            CounterMetric::Messages,
            Granularity::Hour,
            at(10, 0),
            at(11, 0),
        )
        .await
        .unwrap();
    assert_eq!(hours[0].value, 8.0);
    let peaks = store
        .gauges(
            GaugeMetric::SessionsOnline,
            Granularity::Minute,
            at(10, 0),
            at(11, 0),
        )
        .await
        .unwrap();
    assert_eq!(peaks[0].value, 50.0);
}

async fn checkpoints_commit_with_rollups(pool: &PgPool) {
    let store = PgRollupStore::new(pool.clone());
    assert_eq!(
        store.checkpoint("discordy.message.v1", 0).await.unwrap(),
        None
    );
    store
        .apply(&RollupDelta::default(), Some(&checkpoint(10)))
        .await
        .unwrap();
    store
        .apply(&RollupDelta::default(), Some(&checkpoint(25)))
        .await
        .unwrap();
    assert_eq!(
        store.checkpoint("discordy.message.v1", 0).await.unwrap(),
        Some(25)
    );
    assert_eq!(
        store.checkpoint("discordy.message.v1", 1).await.unwrap(),
        None
    );
}

async fn active_users_yield_dau_and_rolling_mau(pool: &PgPool) {
    let store = PgRollupStore::new(pool.clone());
    let mut delta = RollupDelta::default();
    let user = |id: &str| UsageSignal::ActiveUser { user_id: id.into() };
    delta.record(at(9, 0) - Duration::days(1), &user("1"));
    delta.record(at(9, 0), &user("1"));
    delta.record(at(9, 0), &user("2"));
    delta.record(at(9, 0) - Duration::days(40), &user("3"));
    store.apply(&delta, None).await.unwrap();
    store.apply(&delta, None).await.unwrap();

    let today = at(0, 0).date_naive();
    let points = store
        .active_users(today - Duration::days(1), today)
        .await
        .unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!((points[0].dau, points[0].mau), (1, 1));
    assert_eq!((points[1].dau, points[1].mau), (2, 2));
}

async fn voice_quality_sums_are_read_back_per_region(pool: &PgPool) {
    let store = PgRollupStore::new(pool.clone());
    let sample = |region: &str, jitter: f64| {
        UsageSignal::Quality(VoiceQualitySample {
            region: region.into(),
            jitter_ms: jitter,
            loss_pct: 1.0,
            rtt_ms: 30.0,
            bitrate_kbps: 64.0,
        })
    };
    let mut delta = RollupDelta::default();
    delta.record(at(11, 0), &sample("nl-home", 2.0));
    delta.record(at(11, 1), &sample("nl-home", 4.0));
    delta.record(at(11, 1), &sample("de-fra", 9.0));
    store.apply(&delta, None).await.unwrap();

    let regions = store.quality_by_region(at(11, 0), at(11, 5)).await.unwrap();
    assert_eq!(
        regions.iter().map(|(r, _)| r.as_str()).collect::<Vec<_>>(),
        vec!["de-fra", "nl-home"]
    );
    let nl = regions[1].1.averages().unwrap();
    assert_eq!((nl.samples, nl.jitter_ms, nl.rtt_ms), (2, 3.0, 30.0));
}

async fn pruning_drops_expired_minute_buckets(pool: &PgPool) {
    let store = PgRollupStore::new(pool.clone());
    store.prune(at(10, 0) + Duration::days(8)).await.unwrap();
    assert!(
        store
            .counters(
                CounterMetric::Messages,
                Granularity::Minute,
                at(10, 0),
                at(11, 0)
            )
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .counters(
                CounterMetric::Messages,
                Granularity::Hour,
                at(10, 0),
                at(11, 0)
            )
            .await
            .unwrap()
            .len(),
        1
    );
}

async fn feature_flags_upsert_list_and_delete(pool: &PgPool) {
    let flags = PgFeatureFlags::new(pool.clone());
    let flag = |rollout: i64| {
        FeatureFlag::new(
            FlagKey::parse("watch_together").unwrap(),
            "Sync playback",
            true,
            RolloutPercent::parse(rollout).unwrap(),
        )
        .unwrap()
    };
    flags.upsert(&flag(10), "a@discordy.dev").await.unwrap();
    let stored = flags.upsert(&flag(60), "b@discordy.dev").await.unwrap();
    assert_eq!(
        (
            stored.flag.rollout_percent.value(),
            stored.updated_by.as_str()
        ),
        (60, "b@discordy.dev")
    );
    assert_eq!(flags.list().await.unwrap().len(), 1);
    let key = FlagKey::parse("watch_together").unwrap();
    flags.delete(&key).await.unwrap();
    assert_eq!(flags.delete(&key).await, Err(PortError::NotFound));
}

async fn audit_log_pages_newest_first(pool: &PgPool) {
    let audit = PgAuditLog::new(pool.clone());
    for target in ["n1", "n2", "n3"] {
        let entry = NewAuditEntry {
            actor_email: "ops@discordy.dev".into(),
            action: AuditAction::DrainVoiceNode,
            target: target.into(),
            details: r#"{"k":1}"#.into(),
        };
        audit.record(&entry).await.unwrap();
    }
    let page = audit.list(2, None).await.unwrap();
    assert_eq!(
        page.iter().map(|e| e.target.as_str()).collect::<Vec<_>>(),
        vec!["n3", "n2"]
    );
    assert_eq!(page[0].action, "voice_node.drain");
    assert_eq!(page[0].details, r#"{"k": 1}"#);
    let rest = audit.list(2, Some(page[1].id)).await.unwrap();
    assert_eq!(
        rest.iter().map(|e| e.target.as_str()).collect::<Vec<_>>(),
        vec!["n1"]
    );
}
