/// Services always listed on the health page, even before they are scraped.
pub const PLATFORM_SERVICES: [&str; 6] = ["api", "messages", "gateway", "voice-signaling", "sfu", "admin-bff"];

/// Error ratio at or above which a service is considered degraded.
pub const DEGRADED_ERROR_RATIO: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    Healthy,
    Degraded,
    Down,
    Unknown,
}

impl ServiceStatus {
    /// Classifies a service from its scrape targets and 5xx ratio.
    pub fn classify(instances_up: u32, instances_total: u32, error_ratio: Option<f64>) -> Self {
        if instances_total == 0 {
            return ServiceStatus::Unknown;
        }
        if instances_up == 0 {
            return ServiceStatus::Down;
        }
        let erroring = error_ratio.is_some_and(|ratio| ratio >= DEGRADED_ERROR_RATIO);
        if instances_up < instances_total || erroring {
            ServiceStatus::Degraded
        } else {
            ServiceStatus::Healthy
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ServiceStatus::Healthy => "healthy",
            ServiceStatus::Degraded => "degraded",
            ServiceStatus::Down => "down",
            ServiceStatus::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unscraped_services_are_unknown() {
        assert_eq!(ServiceStatus::classify(0, 0, None), ServiceStatus::Unknown);
    }

    #[test]
    fn all_instances_down_is_down() {
        assert_eq!(ServiceStatus::classify(0, 2, Some(0.0)), ServiceStatus::Down);
    }

    #[test]
    fn some_instances_down_is_degraded() {
        assert_eq!(ServiceStatus::classify(1, 2, Some(0.0)), ServiceStatus::Degraded);
    }

    #[test]
    fn high_error_ratio_is_degraded() {
        assert_eq!(ServiceStatus::classify(2, 2, Some(DEGRADED_ERROR_RATIO)), ServiceStatus::Degraded);
    }

    #[test]
    fn all_up_with_few_errors_is_healthy() {
        assert_eq!(ServiceStatus::classify(2, 2, Some(0.01)), ServiceStatus::Healthy);
        assert_eq!(ServiceStatus::classify(1, 1, None), ServiceStatus::Healthy);
    }
}
