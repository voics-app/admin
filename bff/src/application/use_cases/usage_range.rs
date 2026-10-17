use chrono::Duration;

/// Time windows offered by the usage dashboards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageRange {
    Hour,
    Day,
    Week,
    Month,
    Quarter,
}

impl UsageRange {
    pub const ALL: [UsageRange; 5] = [
        UsageRange::Hour,
        UsageRange::Day,
        UsageRange::Week,
        UsageRange::Month,
        UsageRange::Quarter,
    ];

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.as_str() == value)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            UsageRange::Hour => "1h",
            UsageRange::Day => "24h",
            UsageRange::Week => "7d",
            UsageRange::Month => "30d",
            UsageRange::Quarter => "90d",
        }
    }

    pub fn duration(self) -> Duration {
        match self {
            UsageRange::Hour => Duration::hours(1),
            UsageRange::Day => Duration::hours(24),
            UsageRange::Week => Duration::days(7),
            UsageRange::Month => Duration::days(30),
            UsageRange::Quarter => Duration::days(90),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_its_label() {
        for range in UsageRange::ALL {
            assert_eq!(UsageRange::parse(range.as_str()), Some(range));
        }
        assert_eq!(UsageRange::parse("1y"), None);
    }

    #[test]
    fn durations_grow_with_the_range() {
        let durations: Vec<_> = UsageRange::ALL.iter().map(|r| r.duration()).collect();
        assert!(durations.windows(2).all(|w| w[0] < w[1]));
    }
}
