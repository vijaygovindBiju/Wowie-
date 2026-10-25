pub mod linux;
pub mod mock;

use crate::error::EnforcementError;
use crate::model::EnforcementTarget;
use async_trait::async_trait;

#[async_trait]
pub trait EnforcementAdapter: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;
    async fn apply_block(&self, target: &EnforcementTarget) -> Result<(), EnforcementError>;
    async fn remove_block(&self, target: &EnforcementTarget) -> Result<(), EnforcementError>;
    async fn cleanup_all(&self) -> Result<(), EnforcementError>;
}
