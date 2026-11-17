//! 选词模型的后台线程。

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, channel, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use super::finished::Finished;
use super::job::Job;
use super::{ChoiceModel, ChoiceRequest, ComposeRequest};
use crate::engine::rescoring::WakeSlot;

#[cfg(test)]
mod tests;

/// 选词模型的后台线程：最多保留一个待办；过期与卸载时只等待正在执行的单次模型调用。
pub(crate) struct ChoiceWorker {
    pending: Arc<Mutex<Option<(u64, Job)>>>,

    notify: SyncSender<()>,

    results: Receiver<(u64, Finished)>,

    handle: Option<JoinHandle<()>>,

    /// 最近一次请求或取消的序号；0 是初始值。
    latest: Cell<u64>,

    active: Arc<AtomicU64>,
}

impl ChoiceWorker {
    pub fn spawn(model: Arc<dyn ChoiceModel>, wake: WakeSlot) -> Self {
        let (notify, requests) = sync_channel(1);
        let pending = Arc::new(Mutex::new(None::<(u64, Job)>));
        let inbox = pending.clone();
        let (result_tx, results) = channel::<(u64, Finished)>();
        let active = Arc::new(AtomicU64::new(0));
        let current = active.clone();
        let handle = std::thread::Builder::new()
            .name("qingjian-choice".to_owned())
            .spawn(move || {
                while requests.recv().is_ok() {
                    let Some((serial, job)) = inbox.lock().ok().and_then(|mut slot| slot.take())
                    else {
                        continue;
                    };
                    let valid = || current.load(Ordering::Acquire) == serial;
                    if !valid() {
                        continue;
                    }
                    let done = match job {
                        Job::Score(request) => Some(run_score(&*model, request)),
                        Job::Compose(requests) => run_compose(&*model, &requests, valid),
                    };
                    if !valid() {
                        continue;
                    }
                    let Some(done) = done else { continue };
                    if result_tx.send((serial, done)).is_err() {
                        break;
                    }
                    wake.ring();
                }
            })
            .ok();
        if handle.is_none() {
            tracing::warn!("起不了选词模型线程，本次不用模型");
        }
        Self {
            pending,
            notify,
            results,
            handle,
            latest: Cell::new(0),
            active,
        }
    }

    pub fn latest(&self) -> u64 {
        self.latest.get()
    }

    pub fn submit(&self, job: Job) {
        let serial = self.latest.get().wrapping_add(1).max(1);
        self.latest.set(serial);
        self.active.store(serial, Ordering::Release);
        if let Ok(mut pending) = self.pending.lock() {
            *pending = Some((serial, job));
            // 已有通知时待办会被同一轮取走，不阻塞按键线程。
            if matches!(
                self.notify.try_send(()),
                Err(std::sync::mpsc::TrySendError::Disconnected(_))
            ) {
                tracing::warn!("choice worker has stopped");
            }
        }
    }

    pub fn cancel(&self) {
        let serial = self.latest.get().wrapping_add(1).max(1);
        self.latest.set(serial);
        self.active.store(serial, Ordering::Release);
        if let Ok(mut pending) = self.pending.lock() {
            *pending = None;
        }
    }

    pub fn poll(&self) -> Option<(u64, Finished)> {
        self.results
            .try_iter()
            .find(|(serial, _)| *serial == self.latest.get())
    }
}

fn run_score(model: &dyn ChoiceModel, request: ChoiceRequest) -> Finished {
    let texts: Vec<&str> = request.texts.iter().map(String::as_str).collect();
    let started = std::time::Instant::now();
    let logits = model.score(&request.context, &request.typed, &texts);
    let ms = started.elapsed().as_millis() as u64;
    tracing::debug!(texts = texts.len(), ms, "选词模型打分完成");
    Finished::Scored {
        request,
        logits,
        ms,
    }
}

fn run_compose(
    model: &dyn ChoiceModel,
    requests: &[ComposeRequest],
    valid: impl Fn() -> bool,
) -> Option<Finished> {
    let started = std::time::Instant::now();
    let mut tables = Vec::with_capacity(requests.len());
    for request in requests {
        if !valid() {
            return None;
        }
        let syllables: Vec<(&str, bool)> = request
            .syllables
            .iter()
            .map(|(text, complete)| (text.as_str(), *complete))
            .collect();
        let table = model.compose(&request.context, &syllables).map(Arc::new);
        tables.push((request.key.clone(), table));
    }
    tracing::debug!(
        requests = requests.len(),
        ms = started.elapsed().as_millis() as u64,
        "选词模型出字完成"
    );
    Some(Finished::Composed(tables))
}

impl Drop for ChoiceWorker {
    fn drop(&mut self) {
        self.cancel();
        let _ = self.handle.take();
    }
}
