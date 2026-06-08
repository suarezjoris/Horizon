use std::sync::Arc;
use tokio::sync::{Semaphore, OwnedSemaphorePermit};

pub struct VramQueue {
    semaphore: Arc<Semaphore>,
    current_user: Arc<std::sync::Mutex<String>>,
}

impl VramQueue {
    pub fn new() -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(1)),
            current_user: Arc::new(std::sync::Mutex::new(String::new())),
        }
    }

    /// Async acquire — waits until the GPU slot is free.
    pub async fn acquire(&self, label: &str) -> Result<OwnedSemaphorePermit, String> {
        let permit = Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .map_err(|e| e.to_string())?;
        *self.current_user.lock().unwrap() = label.to_string();
        Ok(permit)
    }

    /// Non-blocking try. Returns None if GPU is busy.
    pub fn try_acquire(&self, label: &str) -> Option<OwnedSemaphorePermit> {
        match Arc::clone(&self.semaphore).try_acquire_owned() {
            Ok(permit) => {
                *self.current_user.lock().unwrap() = label.to_string();
                Some(permit)
            }
            Err(_) => None,
        }
    }

    pub fn current_user(&self) -> String {
        self.current_user.lock().unwrap().clone()
    }

    pub fn is_free(&self) -> bool {
        self.semaphore.available_permits() == 1
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
