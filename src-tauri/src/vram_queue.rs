use std::sync::Arc;
use tokio::sync::{Semaphore, OwnedSemaphorePermit};

pub struct VramQueue {
    semaphore: Arc<Semaphore>,
}

impl VramQueue {
    pub fn new() -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    /// Async acquire — waits until the GPU slot is free.
    pub async fn acquire(&self, _label: &str) -> Result<OwnedSemaphorePermit, String> {
        Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .map_err(|e| e.to_string())
    }

    /// Non-blocking try. Returns None if GPU is busy.
    pub fn try_acquire(&self, _label: &str) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.semaphore).try_acquire_owned().ok()
    }

    pub fn semaphore(&self) -> Arc<Semaphore> {
        Arc::clone(&self.semaphore)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_only_one_job_at_a_time() {
        let queue = Arc::new(VramQueue::new());
        let q2 = queue.clone();

        let _permit = queue.acquire("test_job_1").await.unwrap();
        assert!(q2.try_acquire("test_job_2").is_none());
    }

    #[tokio::test]
    async fn test_permit_released_after_drop() {
        let queue = Arc::new(VramQueue::new());
        {
            let _permit = queue.acquire("job_a").await.unwrap();
            assert!(queue.try_acquire("job_b").is_none());
        }
        let _p2 = queue.acquire("job_b").await.unwrap();
        // passes if no hang (timeout would be a failure)
    }
}
