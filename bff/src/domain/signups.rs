use chrono::{DateTime, Utc};

/// Derives signups per bucket from successive peaks of the registered-users gauge.
///
/// The first bucket has no predecessor, so it reports zero. Drops (deleted accounts)
/// are clamped to zero rather than reported as negative signups.
pub fn signups_from_totals(totals: &[(DateTime<Utc>, f64)]) -> Vec<(DateTime<Utc>, i64)> {
    let mut previous: Option<f64> = None;
    totals
        .iter()
        .map(|(bucket, total)| {
            let signups = previous.map_or(0.0, |prev| (total - prev).max(0.0));
            previous = Some(*total);
            (*bucket, signups.round() as i64)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn day(d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, d, 0, 0, 0).unwrap()
    }

    #[test]
    fn no_totals_means_no_signups() {
        assert!(signups_from_totals(&[]).is_empty());
    }

    #[test]
    fn signups_are_differences_between_buckets() {
        let signups = signups_from_totals(&[(day(1), 100.0), (day(2), 104.0), (day(3), 110.0)]);
        assert_eq!(signups, vec![(day(1), 0), (day(2), 4), (day(3), 6)]);
    }

    #[test]
    fn shrinking_totals_report_zero_signups() {
        let signups = signups_from_totals(&[(day(1), 100.0), (day(2), 98.0)]);
        assert_eq!(signups, vec![(day(1), 0), (day(2), 0)]);
    }
}
