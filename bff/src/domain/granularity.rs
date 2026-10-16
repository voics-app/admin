use chrono::{DateTime, Duration, Utc};

/// The resolution of a usage rollup bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Granularity {
    Minute,
    Hour,
    Day,
}

impl Granularity {
    pub const ALL: [Granularity; 3] = [Granularity::Minute, Granularity::Hour, Granularity::Day];

    /// Start of the bucket containing `at`.
    pub fn bucket_start(self, at: DateTime<Utc>) -> DateTime<Utc> {
        let step = self.step().num_seconds();
        let floored = at.timestamp().div_euclid(step) * step;
        DateTime::from_timestamp(floored, 0).unwrap_or(at)
    }

    pub fn step(self) -> Duration {
        match self {
            Granularity::Minute => Duration::minutes(1),
            Granularity::Hour => Duration::hours(1),
            Granularity::Day => Duration::days(1),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Granularity::Minute => "minute",
            Granularity::Hour => "hour",
            Granularity::Day => "day",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.as_str() == value)
    }

    /// The finest granularity that keeps a chart of `range` readable (at most ~200 points).
    pub fn for_range(range: Duration) -> Self {
        const MAX_POINTS: i64 = 200;
        Self::ALL
            .into_iter()
            .find(|g| range.num_seconds() / g.step().num_seconds() <= MAX_POINTS)
            .unwrap_or(Granularity::Day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 16, h, m, s).unwrap()
    }

    #[test]
    fn minute_bucket_drops_seconds() {
        assert_eq!(Granularity::Minute.bucket_start(at(13, 37, 59)), at(13, 37, 0));
    }

    #[test]
    fn hour_bucket_drops_minutes_and_seconds() {
        assert_eq!(Granularity::Hour.bucket_start(at(13, 37, 59)), at(13, 0, 0));
    }

    #[test]
    fn day_bucket_is_utc_midnight() {
        assert_eq!(Granularity::Day.bucket_start(at(23, 59, 59)), at(0, 0, 0));
    }

    #[test]
    fn bucket_start_ignores_subsecond_precision() {
        let precise = at(13, 37, 1) + Duration::milliseconds(999);
        assert_eq!(Granularity::Minute.bucket_start(precise), at(13, 37, 0));
    }

    #[test]
    fn steps_match_bucket_sizes() {
        assert_eq!(Granularity::Minute.step(), Duration::minutes(1));
        assert_eq!(Granularity::Hour.step(), Duration::hours(1));
        assert_eq!(Granularity::Day.step(), Duration::days(1));
    }

    #[test]
    fn round_trips_through_its_name() {
        for g in Granularity::ALL {
            assert_eq!(Granularity::parse(g.as_str()), Some(g));
        }
        assert_eq!(Granularity::parse("week"), None);
    }

    #[test]
    fn picks_minutes_for_short_ranges_and_coarser_buckets_for_long_ones() {
        assert_eq!(Granularity::for_range(Duration::hours(1)), Granularity::Minute);
        assert_eq!(Granularity::for_range(Duration::hours(3)), Granularity::Minute);
        assert_eq!(Granularity::for_range(Duration::hours(24)), Granularity::Hour);
        assert_eq!(Granularity::for_range(Duration::days(7)), Granularity::Hour);
        assert_eq!(Granularity::for_range(Duration::days(30)), Granularity::Day);
        assert_eq!(Granularity::for_range(Duration::days(90)), Granularity::Day);
    }
}
