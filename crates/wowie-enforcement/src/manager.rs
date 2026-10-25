use crate::adapter::EnforcementAdapter;
use crate::error::EnforcementError;
use crate::model::{ActiveBlock, EnforcementProposal, EnforcementTarget};
use crate::policy::EnforcementPolicyEngine;
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct EnforcementManager {
    policy: Arc<EnforcementPolicyEngine>,
    adapter: Arc<dyn EnforcementAdapter>,
    active_blocks: Arc<RwLock<Vec<ActiveBlock>>>,
}

impl EnforcementManager {
    pub fn new(policy: Arc<EnforcementPolicyEngine>, adapter: Arc<dyn EnforcementAdapter>) -> Self {
        Self {
            policy,
            adapter,
            active_blocks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn policy(&self) -> &EnforcementPolicyEngine {
        &self.policy
    }

    pub fn adapter_name(&self) -> &'static str {
        self.adapter.name()
    }

    /// Step 1: Propose an enforcement action (requires explicit user confirmation).
    pub fn propose_block(
        &self,
        target: EnforcementTarget,
        duration_seconds: u32,
        rationale: &str,
        now: DateTime<Utc>,
    ) -> Result<EnforcementProposal, EnforcementError> {
        self.policy
            .propose_block(target, duration_seconds, rationale, now)
    }

    /// Step 2: Confirm a proposal token explicitly.
    pub async fn confirm_block(
        &self,
        token_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<ActiveBlock, EnforcementError> {
        let proposal = self.policy.authorize_proposal(token_id, now)?;

        // Apply block via adapter
        self.adapter.apply_block(&proposal.target).await?;

        let block = ActiveBlock {
            id: Uuid::new_v4(),
            token_id,
            target: proposal.target,
            enforced_at: now,
            expires_at: now + Duration::seconds(i64::from(proposal.duration_seconds)),
            mechanism: self.adapter.name().to_string(),
        };

        let mut blocks = self.active_blocks.write().await;
        blocks.push(block.clone());

        Ok(block)
    }

    /// Periodic expiration check. Removes any blocks that have expired.
    pub async fn sweep_expired(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<ActiveBlock>, EnforcementError> {
        let mut blocks = self.active_blocks.write().await;
        let mut expired = Vec::new();
        let mut remaining = Vec::new();

        for block in blocks.drain(..) {
            if block.is_expired(now) {
                self.adapter.remove_block(&block.target).await?;
                expired.push(block);
            } else {
                remaining.push(block);
            }
        }

        *blocks = remaining;
        Ok(expired)
    }

    /// User stop control: manually cancels all active enforcement actions immediately.
    pub async fn stop_all(&self) -> Result<(), EnforcementError> {
        let mut blocks = self.active_blocks.write().await;
        blocks.clear();
        self.adapter.cleanup_all().await
    }

    pub async fn list_active_blocks(&self) -> Vec<ActiveBlock> {
        self.active_blocks.read().await.clone()
    }
}
