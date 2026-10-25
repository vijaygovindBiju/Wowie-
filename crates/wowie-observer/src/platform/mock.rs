use super::PlatformObserver;
use crate::error::ObserverError;
use crate::model::WindowMetadata;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct MockObserver {
    queue: Arc<Mutex<VecDeque<WindowMetadata>>>,
    fixed_metadata: Arc<Mutex<Option<WindowMetadata>>>,
}

impl MockObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_fixed(metadata: WindowMetadata) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            fixed_metadata: Arc::new(Mutex::new(Some(metadata))),
        }
    }

    pub fn enqueue(&self, metadata: WindowMetadata) {
        self.queue.lock().unwrap().push_back(metadata);
    }

    pub fn set_fixed(&self, metadata: Option<WindowMetadata>) {
        *self.fixed_metadata.lock().unwrap() = metadata;
    }
}

#[async_trait]
impl PlatformObserver for MockObserver {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn sample_active_window(&self) -> Result<WindowMetadata, ObserverError> {
        let queued = self.queue.lock().unwrap().pop_front();
        if let Some(item) = queued {
            return Ok(item);
        }

        let fixed = self.fixed_metadata.lock().unwrap().clone();
        if let Some(item) = fixed {
            return Ok(item);
        }

        Ok(WindowMetadata::empty())
    }
}
