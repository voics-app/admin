use std::collections::BTreeMap;

/// A voice server as registered in etcd (P8).
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceNode {
    pub node_id: String,
    pub region: String,
    pub ws_url: String,
    pub public_ip: String,
    pub udp_port: u16,
    pub users: u64,
    pub cpu_pct: f64,
    pub egress_bps: u64,
    pub draining: bool,
    pub version: String,
}

/// Aggregate load of one voice region.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionLoad {
    pub region: String,
    pub nodes: usize,
    pub accepting_nodes: usize,
    pub draining_nodes: usize,
    pub users: u64,
    pub egress_bps: u64,
    pub avg_cpu_pct: f64,
    pub max_cpu_pct: f64,
}

/// Groups nodes by region, ordered by region name.
pub fn summarize_regions(nodes: &[VoiceNode]) -> Vec<RegionLoad> {
    let mut by_region: BTreeMap<&str, Vec<&VoiceNode>> = BTreeMap::new();
    for node in nodes {
        by_region.entry(node.region.as_str()).or_default().push(node);
    }
    by_region.into_iter().map(|(region, members)| summarize(region, &members)).collect()
}

fn summarize(region: &str, members: &[&VoiceNode]) -> RegionLoad {
    let draining_nodes = members.iter().filter(|n| n.draining).count();
    let cpu_total: f64 = members.iter().map(|n| n.cpu_pct).sum();
    RegionLoad {
        region: region.to_owned(),
        nodes: members.len(),
        accepting_nodes: members.len() - draining_nodes,
        draining_nodes,
        users: members.iter().map(|n| n.users).sum(),
        egress_bps: members.iter().map(|n| n.egress_bps).sum(),
        avg_cpu_pct: cpu_total / members.len() as f64,
        max_cpu_pct: members.iter().map(|n| n.cpu_pct).fold(0.0, f64::max),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, region: &str, users: u64, cpu: f64, draining: bool) -> VoiceNode {
        VoiceNode {
            node_id: id.into(),
            region: region.into(),
            ws_url: format!("wss://discordy-rtc-{region}.voics.nl"),
            public_ip: "203.0.113.10".into(),
            udp_port: 50000,
            users,
            cpu_pct: cpu,
            egress_bps: users * 1_000,
            draining,
            version: "0.1.0".into(),
        }
    }

    #[test]
    fn no_nodes_means_no_regions() {
        assert!(summarize_regions(&[]).is_empty());
    }

    #[test]
    fn sums_load_per_region() {
        let regions = summarize_regions(&[
            node("a", "nl-home", 10, 20.0, false),
            node("b", "nl-home", 30, 60.0, true),
            node("c", "de-fra", 5, 10.0, false),
        ]);
        assert_eq!(regions.len(), 2);
        let nl = &regions[1];
        assert_eq!(nl.region, "nl-home");
        assert_eq!(nl.nodes, 2);
        assert_eq!(nl.accepting_nodes, 1);
        assert_eq!(nl.draining_nodes, 1);
        assert_eq!(nl.users, 40);
        assert_eq!(nl.egress_bps, 40_000);
        assert_eq!(nl.avg_cpu_pct, 40.0);
        assert_eq!(nl.max_cpu_pct, 60.0);
    }

    #[test]
    fn regions_are_sorted_by_name() {
        let regions = summarize_regions(&[node("a", "us-east", 1, 1.0, false), node("b", "ap-south", 1, 1.0, false)]);
        let names: Vec<_> = regions.iter().map(|r| r.region.as_str()).collect();
        assert_eq!(names, vec!["ap-south", "us-east"]);
    }
}
