use crate::error::EnforcementError;
use crate::model::{EnforcementProposal, EnforcementRule, EnforcementTarget};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub const MAX_BLOCK_DURATION_SECONDS: u32 = 7200; // 2 hours max
pub const MIN_BLOCK_DURATION_SECONDS: u32 = 60; // 1 minute min
pub const PROPOSAL_VALIDITY_SECONDS: i64 = 120; // 2 minutes to confirm

pub const PROTECTED_DOMAINS: &[&str] = &[
    "github.com",
    "crates.io",
    "docs.rs",
    "rust-lang.org",
    "archlinux.org",
    "stackoverflow.com",
    "localhost",
    "127.0.0.1",
];

pub const PROTECTED_APPS: &[&str] = &[
    "code",
    "vscode",
    "rustc",
    "cargo",
    "bash",
    "sh",
    "zsh",
    "git",
    "kitty",
    "foot",
    "alacritty",
    "wezterm",
    "terminal",
    "hyprland",
    "waybar",
    "mako",
    "dunst",
];

#[derive(Debug, Clone)]
pub struct EnforcementPolicyEngine {
    configured_rules: Arc<Mutex<Vec<EnforcementRule>>>,
    pending_proposals: Arc<Mutex<HashMap<Uuid, EnforcementProposal>>>,
}

impl Default for EnforcementPolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EnforcementPolicyEngine {
    pub fn new() -> Self {
        Self {
            configured_rules: Arc::new(Mutex::new(Vec::new())),
            pending_proposals: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn is_essential_tool(target: &EnforcementTarget) -> bool {
        let val = target.raw_value().to_lowercase();
        match target {
            EnforcementTarget::Domain(_) => PROTECTED_DOMAINS
                .iter()
                .any(|&d| val == d || val.ends_with(&format!(".{d}"))),
            EnforcementTarget::AppClass(_) => {
                PROTECTED_APPS.iter().any(|&a| val == a || val.contains(a))
            }
        }
    }

    pub fn add_rule(&self, target: EnforcementTarget) -> EnforcementRule {
        let rule = EnforcementRule {
            id: Uuid::new_v4(),
            target,
            is_enabled: true,
            created_at: Utc::now(),
        };
        self.configured_rules.lock().unwrap().push(rule.clone());
        rule
    }

    pub fn list_rules(&self) -> Vec<EnforcementRule> {
        self.configured_rules.lock().unwrap().clone()
    }

    /// Proposes a bounded, reversible block. Does NOT activate the block.
    /// Explicit confirmation with the generated token is required.
    pub fn propose_block(
        &self,
        target: EnforcementTarget,
        duration_seconds: u32,
        rationale: &str,
        now: DateTime<Utc>,
    ) -> Result<EnforcementProposal, EnforcementError> {
        if Self::is_essential_tool(&target) {
            return Err(EnforcementError::EssentialToolProtected(
                target.raw_value().to_string(),
            ));
        }

        let bounded_duration =
            duration_seconds.clamp(MIN_BLOCK_DURATION_SECONDS, MAX_BLOCK_DURATION_SECONDS);

        let token_id = Uuid::new_v4();
        let proposal = EnforcementProposal {
            token_id,
            target,
            duration_seconds: bounded_duration,
            rationale: rationale.to_string(),
            created_at: now,
            expires_at: now + Duration::seconds(PROPOSAL_VALIDITY_SECONDS),
        };

        self.pending_proposals
            .lock()
            .unwrap()
            .insert(token_id, proposal.clone());

        Ok(proposal)
    }

    /// Explicitly authorize and consume a proposed block token.
    pub fn authorize_proposal(
        &self,
        token_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<EnforcementProposal, EnforcementError> {
        let proposal = self
            .pending_proposals
            .lock()
            .unwrap()
            .remove(&token_id)
            .ok_or(EnforcementError::UnauthorizedToken)?;

        if proposal.is_expired(now) {
            return Err(EnforcementError::UnauthorizedToken);
        }

        Ok(proposal)
    }
}
