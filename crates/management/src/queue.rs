use crate::{OperationEvent, storage::save_json};
use async_channel::{Receiver, Sender};
use containerhub_core::*;
use std::{path::Path, sync::Arc};
mod checkpoint;
mod control;
mod execution;
use control::{cancel, reconcile};

pub enum OperationRequest {
    Submit(Box<Operation>, SecretEnvironment),
    Cancel {
        id: u64,
        reply: tokio::sync::oneshot::Sender<Result<Operation, ManagementError>>,
    },
    Reconcile {
        id: u64,
        reply: tokio::sync::oneshot::Sender<Result<Operation, ManagementError>>,
    },
}
impl From<Operation> for OperationRequest {
    fn from(op: Operation) -> Self {
        Self::Submit(Box::new(op), SecretEnvironment::default())
    }
}

pub async fn operations(
    provider: Arc<dyn ManagementClient>,
    data: &Path,
    rx: Receiver<OperationRequest>,
    events: Sender<OperationEvent>,
) {
    use futures_util::{StreamExt, stream::FuturesUnordered};
    use std::{
        collections::{HashMap, HashSet},
        future::Future,
        pin::Pin,
    };
    type Running = Pin<Box<dyn Future<Output = Operation> + Send>>;
    let path = data.join("operations.json");
    let mut history: Vec<Operation> = match std::fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(history) => history,
            Err(_) => {
                let _ = events.send(OperationEvent::WriterUnavailable).await;
                return;
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(_) => {
            let _ = events.send(OperationEvent::WriterUnavailable).await;
            return;
        }
    };
    let mut queue = crate::recovery::restore(provider.as_ref(), &mut history).await;
    if !history.is_empty() && save_operations(&path, &history).await.is_err() {
        let _ = events.send(OperationEvent::WriterUnavailable).await;
        return;
    }
    for op in &history {
        if events
            .send(OperationEvent::Operation(Box::new(op.clone())))
            .await
            .is_err()
        {
            return;
        }
    }
    if events.send(OperationEvent::Ready).await.is_err() {
        return;
    }
    let mut running: FuturesUnordered<Running> = FuturesUnordered::new();
    let mut busy = HashSet::new();
    let mut inputs: HashMap<u64, SecretEnvironment> = HashMap::new();
    let mut accepting = true;
    let mut controls: HashMap<u64, ExecutionControl> = HashMap::new();
    let (checkpoints, checkpoint_rx) = async_channel::bounded(4);
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(200));
    loop {
        while running.len() < 4 {
            let Some(index) = queue
                .iter()
                .position(|op| !busy.contains(&op.command.lock_key()))
            else {
                break;
            };
            let mut op = queue.remove(index).expect("known queued operation");
            let recovering = op.state == OperationState::Reconciling;
            let input = inputs.remove(&op.id).unwrap_or_default();
            if !recovering {
                match provider.prepare_execution(&op.command) {
                    Ok(receipt) => op.receipt = receipt,
                    Err(error) => {
                        op.state = OperationState::Failed;
                        op.finished_at = Some(now());
                        op.message = Some(error.message);
                        replace_operation(&mut history, &op);
                        if save_operations(&path, &history).await.is_err() {
                            let _ = events.send(OperationEvent::StorageError).await;
                        }
                        let _ = events.send(OperationEvent::Operation(Box::new(op))).await;
                        continue;
                    }
                }
                op.state = OperationState::Running;
            }
            replace_operation(&mut history, &op);
            if save_operations(&path, &history).await.is_err() {
                op.state = if recovering {
                    OperationState::UnknownOutcome
                } else {
                    OperationState::Failed
                };
                op.finished_at = Some(now());
                op.message = Some("无法保存执行/核对记录，未发送请求".into());
                replace_operation(&mut history, &op);
                let _ = events.send(OperationEvent::Operation(Box::new(op))).await;
                continue;
            }
            busy.insert(op.command.lock_key());
            if events
                .send(OperationEvent::Operation(Box::new(op.clone())))
                .await
                .is_err()
            {
                return;
            }
            let provider = provider.clone();
            let control = ExecutionControl::with_checkpoint(
                input,
                Arc::new(checkpoint::Journal {
                    id: op.id,
                    tx: checkpoints.clone(),
                }),
            );
            controls.insert(op.id, control.clone());
            if op.cancel_requested {
                control.cancel();
            }
            running.push(Box::pin(execution::run(provider, op, recovering, control)));
        }
        if !accepting && running.is_empty() && queue.is_empty() {
            return;
        }
        tokio::select! {
            checkpoint=checkpoint_rx.recv() => if let Ok(checkpoint) = checkpoint {
                let result = checkpoint::save(&path, &mut history, &checkpoint).await;
                if let Ok(op) = &result { let _ = events.send(OperationEvent::Operation(Box::new(op.clone()))).await; }
                let _ = checkpoint.reply.send(result.map(|_| ()));
            },
            request=rx.recv(), if accepting => match request {
                Ok(OperationRequest::Submit(op, input))=>{
                    let mut op = *op;
                    if let Err(error)=op.command.validate() { op.state=OperationState::Failed;op.message=Some(error.message);op.finished_at=Some(now());let _=events.send(OperationEvent::Operation(Box::new(op))).await;continue; }

                    if let Some(existing)=history.iter().find(|old|old.id==op.id) {
                        // A duplicate is never executed again. The UI generates a new ID for explicit retries.
                        if existing.command==op.command && input.is_empty() {let _=events.send(OperationEvent::Operation(Box::new(existing.clone()))).await;}
                        else {let _=events.send(OperationEvent::StorageError).await;}
                        continue;
                    }
                    if let Err(error) = input.validate_for(&op.command) { op.state=OperationState::Failed;op.message=Some(error.message);op.finished_at=Some(now());let _=events.send(OperationEvent::Operation(Box::new(op))).await;continue; }
                    if queue.len()>=32 { op.state=OperationState::Failed;op.finished_at=Some(now());op.message=Some("操作队列已满，未执行命令".into()); let _=events.send(OperationEvent::Operation(Box::new(op))).await;continue; }
                    if history.len()>=crate::MAX_OPERATIONS {op.state=OperationState::Failed;op.message=Some("任务日志已达容量上限，需要维护归档".into());op.finished_at=Some(now());let _=events.send(OperationEvent::Operation(Box::new(op))).await;continue;}
                    op.state=OperationState::Queued;
                    history.push(op.clone());
                    if save_operations(&path,&history).await.is_err() {
                        op.state=OperationState::Failed;op.finished_at=Some(now());op.message=Some("无法保存操作记录，未执行命令".into());
                        replace_operation(&mut history,&op);let _=events.send(OperationEvent::Operation(Box::new(op))).await;continue;
                    }
                    // Only a durable acceptance emits Queued. Submitting remains an in-memory UI intent.
                    inputs.insert(op.id, input);
                    queue.push_back(op.clone());
                    if events.send(OperationEvent::Operation(Box::new(op))).await.is_err(){return;}
                },
                Ok(OperationRequest::Reconcile { id, reply }) => {
                    let result = reconcile(id, &path, &mut history, &mut queue).await;
                    if let Ok(op) = &result { let _ = events.send(OperationEvent::Operation(Box::new(op.clone()))).await; }
                    if let Err(error) = &result { let _ = events.send(OperationEvent::ControlError(error.message.clone())).await; }
                    let _ = reply.send(result);
                }
                Ok(OperationRequest::Cancel { id, reply }) => {
                    let result = cancel(id, &path, &mut history, &mut queue, &controls).await;
                    if let Ok(op) = &result { let _ = events.send(OperationEvent::Operation(Box::new(op.clone()))).await; }
                    if let Err(error) = &result { let _ = events.send(OperationEvent::ControlError(error.message.clone())).await; }
                    if result.as_ref().is_ok_and(|op| !op.state.is_active()) { inputs.remove(&id); }
                    let _ = reply.send(result);
                }
                Err(_)=>accepting=false,
            },
            _=tick.tick(), if !controls.is_empty()=>{
                for op in history.iter_mut().filter(|op| op.state.is_active()) {
                    if let Some(control) = controls.get(&op.id) {
                        let progress = control.progress();
                        if op.progress != progress { op.progress = progress; let _=events.send(OperationEvent::Operation(Box::new(op.clone()))).await; }
                    }
                }
            },
            completed=running.next(), if !running.is_empty()=>{
                if let Some(mut op)=completed {
                    controls.remove(&op.id);
                    busy.remove(&op.command.lock_key());
                    replace_operation(&mut history,&op);
                    if save_operations(&path,&history).await.is_err() {
                        op.state=OperationState::UnknownOutcome;
                        op.message=Some("命令已返回，但无法保存结果；请核对实际状态".into());
                        replace_operation(&mut history,&op);let _=events.send(OperationEvent::StorageError).await;
                    }
                    if events.send(OperationEvent::Operation(Box::new(op))).await.is_err(){return;}
                }
            },
        }
    }
}
fn replace_operation(history: &mut [Operation], operation: &Operation) {
    if let Some(old) = history.iter_mut().find(|old| old.id == operation.id) {
        *old = operation.clone();
    }
}
async fn save_operations(path: &Path, history: &[Operation]) -> std::io::Result<()> {
    let path = path.to_owned();
    let history = history.to_vec();
    tokio::task::spawn_blocking(move || save_json(&path, &history))
        .await
        .map_err(std::io::Error::other)?
}
