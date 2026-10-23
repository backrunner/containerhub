use crate::OperationEvent;
use containerhub_core::*;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

#[derive(Default)]
struct State {
    ready: bool,
    closed: bool,
    failed: bool,
    records: HashMap<u64, Operation>,
}
pub struct OperationService {
    writes: async_channel::Sender<crate::queue::OperationRequest>,
    state: Arc<Mutex<State>>,
    updates: watch::Sender<u64>,
    submissions: tokio::sync::Mutex<()>,
}
fn unavailable() -> ManagementError {
    ManagementError::new(ErrorKind::Unavailable, "任务日志或执行服务不可用")
}
impl OperationService {
    pub async fn open(
        provider: Arc<dyn ManagementClient>,
        directory: PathBuf,
    ) -> Result<Self, ManagementError> {
        let (writes, requests) = async_channel::bounded(32);
        let (events, receiver) = async_channel::bounded(64);
        let state = Arc::new(Mutex::new(State::default()));
        let (updates, _) = watch::channel(0);
        tokio::spawn(async move {
            crate::queue::operations(provider, &directory, requests, events).await
        });
        let tracker = state.clone();
        let changes = updates.clone();
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                {
                    let mut state = tracker.lock().expect("operation tracker");
                    match event {
                        OperationEvent::ControlError(_) => {}
                        OperationEvent::Ready => state.ready = true,
                        OperationEvent::Operation(operation) => {
                            state.records.insert(operation.id, *operation);
                        }
                        OperationEvent::StorageError | OperationEvent::WriterUnavailable => {
                            state.failed = true
                        }
                    }
                }
                changes.send_modify(|version| *version += 1);
            }
            tracker.lock().expect("operation tracker").closed = true;
            changes.send_modify(|version| *version += 1);
        });
        let service = Self {
            writes,
            state,
            updates,
            submissions: Default::default(),
        };
        let mut changes = service.updates.subscribe();
        loop {
            {
                let state = service.state.lock().map_err(|_| unavailable())?;
                if state.failed || state.closed {
                    return Err(unavailable());
                }
                if state.ready {
                    break;
                }
            }
            changes.changed().await.map_err(|_| unavailable())?;
        }
        Ok(service)
    }
    pub fn get(&self, id: u64) -> Result<Operation, ManagementError> {
        self.state
            .lock()
            .map_err(|_| unavailable())?
            .records
            .get(&id)
            .cloned()
            .ok_or_else(|| {
                ManagementError::new(ErrorKind::NotFound, "未找到该操作 ID；不会自动重新提交")
            })
    }
    pub fn recent(&self) -> Result<Vec<Operation>, ManagementError> {
        let mut records: Vec<_> = self
            .state
            .lock()
            .map_err(|_| unavailable())?
            .records
            .values()
            .cloned()
            .collect();
        records.sort_by_key(|op| std::cmp::Reverse((op.started_at, op.id)));
        records.truncate(100);
        Ok(records)
    }
    pub async fn submit(&self, id: u64, command: Command) -> Result<Operation, ManagementError> {
        self.submit_with_secrets(id, command, SecretEnvironment::default())
            .await
    }
    pub async fn submit_with_secrets(
        &self,
        id: u64,
        command: Command,
        secrets: SecretEnvironment,
    ) -> Result<Operation, ManagementError> {
        command.validate()?;
        if id == 0 || id > 9_007_199_254_740_991 {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                "操作 ID 必须是正的 JavaScript 安全整数",
            ));
        }
        // Serialize acceptance only; running tasks remain concurrent and reads do not take this lock.
        let _accept = self.submissions.lock().await;
        {
            let state = self.state.lock().map_err(|_| unavailable())?;
            if state.failed || state.closed {
                return Err(unavailable());
            }
            if let Some(old) = state.records.get(&id) {
                return if old.command == command && secrets.is_empty() {
                    Ok(old.clone())
                } else {
                    Err(ManagementError::new(
                        ErrorKind::Conflict,
                        "操作 ID 已被使用；秘密输入只能提交一次，请查询原任务 ID",
                    ))
                };
            }
            if state.records.len() >= crate::MAX_OPERATIONS {
                return Err(ManagementError::new(
                    ErrorKind::Conflict,
                    "任务日志已达容量上限，需要维护归档",
                ));
            }
        }
        secrets.validate_for(&command)?;
        let mut changes = self.updates.subscribe();
        self.writes
            .try_send(crate::queue::OperationRequest::Submit(
                Box::new(Operation {
                    id,
                    command,
                    receipt: None,
                    result: None,
                    progress: None,
                    cancel_requested: false,
                    state: OperationState::Submitting,
                    started_at: now(),
                    finished_at: None,
                    message: None,
                }),
                secrets,
            ))
            .map_err(|_| ManagementError::new(ErrorKind::Conflict, "操作队列已满或已关闭"))?;
        loop {
            {
                let state = self.state.lock().map_err(|_| unavailable())?;
                if let Some(operation) = state.records.get(&id) {
                    return Ok(operation.clone());
                }
                if state.failed || state.closed {
                    return Err(unavailable());
                }
            }
            changes.changed().await.map_err(|_| unavailable())?;
        }
    }
    pub async fn cancel(&self, id: u64) -> Result<Operation, ManagementError> {
        let (reply, response) = tokio::sync::oneshot::channel();
        self.writes
            .try_send(crate::queue::OperationRequest::Cancel { id, reply })
            .map_err(|_| unavailable())?;
        response.await.map_err(|_| unavailable())?
    }
    pub async fn reconcile(&self, id: u64) -> Result<Operation, ManagementError> {
        let (reply, response) = tokio::sync::oneshot::channel();
        self.writes
            .try_send(crate::queue::OperationRequest::Reconcile { id, reply })
            .map_err(|_| unavailable())?;
        response.await.map_err(|_| unavailable())?
    }
    pub async fn shutdown(&self) {
        self.writes.close();
        let mut changes = self.updates.subscribe();
        loop {
            if self.state.lock().expect("operation tracker").closed {
                return;
            }
            if changes.changed().await.is_err() {
                return;
            }
        }
    }
}
