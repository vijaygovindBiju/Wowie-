use super::EnforcementAdapter;
use crate::error::EnforcementError;
use crate::model::EnforcementTarget;
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default, Clone)]
pub struct MockEnforcementAdapter {
    applied_blocks: Arc<Mutex<HashSet<EnforcementTarget>>>,
}

impl MockEnforcementAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_blocked(&self, target: &EnforcementTarget) -> bool {
        self.applied_blocks.lock().unwrap().contains(target)
    }

    pub fn block_count(&self) -> usize {
        self.applied_blocks.lock().unwrap().len()
    }
}

#[async_trait]
impl EnforcementAdapter for MockEnforcementAdapter {
    fn name(&self) -> &'static str {
        "mock"
    }

    async fn apply_block(&self, target: &EnforcementTarget) -> Result<(), EnforcementError> {
        self.applied_blocks.lock().unwrap().insert(target.clone());
        Ok(())
    }

    async fn remove_block(&self, target: &EnforcementTarget) -> Result<(), EnforcementError> {
        self.applied_blocks.lock().unwrap().remove(target);
        Ok(())
    }

    async fn cleanup_all(&self) -> Result<(), EnforcementError> {
        self.applied_blocks.lock().unwrap().clear();
        Ok(())
    }
}
