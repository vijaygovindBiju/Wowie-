use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum EnforcementTarget {
    Domain(String),
    AppClass(String),
}

impl std::fmt::Display for EnforcementTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Domain(d) => write!(f, "domain:{d}"),
            Self::AppClass(a) => write!(f, "app:{a}"),
        }
    }
}

impl EnforcementTarget {
    pub fn raw_value(&self) -> &str {
        match self {
            Self::Domain(d) | Self::AppClass(d) => d.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnforcementRule {
    pub id: Uuid,
    pub target: EnforcementTarget,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnforcementProposal {
    pub token_id: Uuid,
    pub target: EnforcementTarget,
    pub duration_seconds: u32,
    pub rationale: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl EnforcementProposal {
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveBlock {
    pub id: Uuid,
    pub token_id: Uuid,
    pub target: EnforcementTarget,
    pub enforced_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub mechanism: String,
}

impl ActiveBlock {
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }
}
