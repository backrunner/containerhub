use containerhub_core::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::{OwnedMutexGuard, OwnedSemaphorePermit, Semaphore};

/// Runtime-agnostic bounded scheduling; authorization and durable journals belong to the caller.
pub struct Scheduler {
    pending: Arc<Semaphore>,
    running: Arc<Semaphore>,
    resources: Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>,
}
pub struct Reservation {
    _pending: OwnedSemaphorePermit,
    resource: Arc<tokio::sync::Mutex<()>>,
    running: Arc<Semaphore>,
}
pub struct ExecutionPermit {
    _reservation: Reservation,
    _resource: OwnedMutexGuard<()>,
    _running: OwnedSemaphorePermit,
}
impl Default for Scheduler {
    fn default() -> Self {
        Self {
            pending: Arc::new(Semaphore::new(32)),
            running: Arc::new(Semaphore::new(4)),
            resources: Mutex::new(HashMap::new()),
        }
    }
}
impl Scheduler {
    pub fn reserve(&self, command: &Command) -> Result<Reservation, ManagementError> {
        command.validate()?;
        let permit =
            self.pending.clone().try_acquire_owned().map_err(|_| {
                ManagementError::new(ErrorKind::Conflict, "操作队列已满，请稍后重试")
            })?;
        let mut resources = self
            .resources
            .lock()
            .map_err(|_| ManagementError::new(ErrorKind::Unavailable, "资源调度器不可用"))?;
        resources.retain(|_, weak| weak.strong_count() > 0);
        let key = command.lock_key();
        let resource = resources
            .get(&key)
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| {
                let lock = Arc::new(tokio::sync::Mutex::new(()));
                resources.insert(key, Arc::downgrade(&lock));
                lock
            });
        Ok(Reservation {
            _pending: permit,
            resource,
            running: self.running.clone(),
        })
    }
}
impl Reservation {
    pub async fn enter(self) -> ExecutionPermit {
        let resource = self.resource.clone().lock_owned().await;
        let running = self
            .running
            .clone()
            .acquire_owned()
            .await
            .expect("scheduler never closed");
        ExecutionPermit {
            _reservation: self,
            _resource: resource,
            _running: running,
        }
    }
}
