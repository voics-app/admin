-- Usage rollups for the admin portal. Counts only: no message content is ever stored.
-- Counters are summed per bucket; gauges keep the peak. Metric names are defined by the BFF.

CREATE TABLE usage_rollup_minute (
    metric     TEXT             NOT NULL,
    dimension  TEXT             NOT NULL,
    bucket     TIMESTAMPTZ      NOT NULL,
    value      DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (metric, bucket, dimension)
);

CREATE TABLE usage_rollup_hour (LIKE usage_rollup_minute INCLUDING ALL);
CREATE TABLE usage_rollup_day (LIKE usage_rollup_minute INCLUDING ALL);

-- Distinct active users per UTC day, for DAU and rolling 30-day MAU.
CREATE TABLE active_users_daily (
    day     DATE NOT NULL,
    user_id TEXT NOT NULL,
    PRIMARY KEY (day, user_id)
);

-- Aggregator position per partition, committed in the same transaction as the rollups.
CREATE TABLE consumer_offsets (
    topic       TEXT        NOT NULL,
    partition   INTEGER     NOT NULL,
    next_offset BIGINT      NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (topic, partition)
);
