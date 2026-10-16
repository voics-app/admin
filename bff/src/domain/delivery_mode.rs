/// Message delivery modes (spec §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DeliveryMode {
    Standard,
    Ephemeral,
    P2p,
}

impl DeliveryMode {
    pub fn as_str(self) -> &'static str {
        match self {
            DeliveryMode::Standard => "standard",
            DeliveryMode::Ephemeral => "ephemeral",
            DeliveryMode::P2p => "p2p",
        }
    }
}
