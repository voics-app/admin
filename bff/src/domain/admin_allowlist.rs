use std::collections::BTreeSet;

/// The second access gate after Cloudflare Access: only these emails may administer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminAllowlist {
    emails: BTreeSet<String>,
}

impl AdminAllowlist {
    /// Parses a comma-separated list. Whitespace and case are ignored; blanks are dropped.
    pub fn parse(csv: &str) -> Self {
        let emails = csv
            .split(',')
            .map(normalize)
            .filter(|e| !e.is_empty())
            .collect();
        Self { emails }
    }

    pub fn is_empty(&self) -> bool {
        self.emails.is_empty()
    }

    pub fn permits(&self, email: &str) -> bool {
        self.emails.contains(&normalize(email))
    }
}

fn normalize(email: &str) -> String {
    email.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permits_listed_emails() {
        let list = AdminAllowlist::parse("ops@discordy.dev,owner@discordy.dev");
        assert!(list.permits("owner@discordy.dev"));
    }

    #[test]
    fn rejects_unlisted_emails() {
        let list = AdminAllowlist::parse("ops@discordy.dev");
        assert!(!list.permits("mallory@evil.dev"));
    }

    #[test]
    fn matching_ignores_case_and_surrounding_whitespace() {
        let list = AdminAllowlist::parse("  Ops@Discordy.dev , ");
        assert!(list.permits("ops@discordy.DEV "));
    }

    #[test]
    fn blank_entries_are_dropped_and_never_match_an_empty_email() {
        let list = AdminAllowlist::parse(" , ,");
        assert!(list.is_empty());
        assert!(!list.permits(""));
    }

    #[test]
    fn a_prefix_of_a_listed_email_does_not_match() {
        let list = AdminAllowlist::parse("ops@discordy.dev");
        assert!(!list.permits("ops@discordy.de"));
    }
}
