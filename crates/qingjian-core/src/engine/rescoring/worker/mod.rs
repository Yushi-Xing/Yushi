//! 合并同一查询的各读法，后台一次完成；排队时只保留最新输入任务。

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::thread::JoinHandle;

use crate::sentence::SentenceScorer;

use super::batch::ScoreBatch;
use super::{GENERATE_BEAM, GENERATE_MAX_CHARS};

mod job;
mod scored;
#[cfg(test)]
mod tests;

use job::Job;
pub(crate) use scored::Scored;

/// 后台打分线程：模型前向要几十毫秒，不能放在按键回调里。
/// 任务排队时只算最新的一条（旧的对应已经过去的输入状态）；线程随本结构一起结束。
pub(crate) struct RescoreWorker {
    jobs: Sender<Job>,
    results: Receiver<Scored>,
    handle: Option<JoinHandle<()>>,

    /// 最新输入代；换输入或卸载模型后停止后续批次。
    latest: Arc<AtomicU64>,
}

impl RescoreWorker {
    pub fn spawn(scorer: Box<dyn SentenceScorer>) -> Self {
        let (jobs, job_rx) = channel::<Job>();
        let (result_tx, results) = channel::<Scored>();
        let latest = Arc::new(AtomicU64::new(0));
        let active = latest.clone();
        let handle = std::thread::Builder::new()
            .name("qingjian-rescore".to_owned())
            .spawn(move || {
                while let Ok(mut job) = job_rx.recv() {
                    // 攒了好几条只算最后一条
                    while let Ok(newer) = job_rx.try_recv() {
                        job = newer;
                    }
                    for batch in &mut job.batches {
                        if active.load(Ordering::Acquire) != job.epoch {
                            break;
                        }
                        let texts: Vec<&str> = batch.texts.iter().map(String::as_str).collect();
                        let started = std::time::Instant::now();
                        batch.scores = scorer.score(&job.context, &batch.keys, &texts);
                        tracing::debug!(
                            texts = texts.len(),
                            keys = batch.keys.len(),
                            ms = started.elapsed().as_millis(),
                            "神经重打分完成"
                        );
                    }
                    if active.load(Ordering::Acquire) != job.epoch {
                        continue;
                    }
                    let generated = job.generate.map(|keys| {
                        let started = std::time::Instant::now();
                        let texts = scorer.generate(&keys, GENERATE_BEAM, GENERATE_MAX_CHARS);
                        tracing::debug!(
                            keys = keys.len(),
                            got = texts.len(),
                            ms = started.elapsed().as_millis(),
                            "整句生成完成"
                        );
                        (keys, texts)
                    });
                    let done = Scored {
                        epoch: job.epoch,
                        batches: job.batches,
                        generated,
                    };
                    if active.load(Ordering::Acquire) != done.epoch {
                        continue;
                    }
                    if result_tx.send(done).is_err() {
                        break;
                    }
                }
            })
            .ok();
        if handle.is_none() {
            tracing::warn!("起不了神经重打分线程，本次不用模型");
        }
        Self {
            jobs,
            results,
            handle,
            latest,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.handle.is_some()
    }

    /// 输入变了就取消旧批次，不等下一次防抖请求才停止。
    pub fn cancel_outdated(&self, epoch: u64) {
        self.latest.store(epoch, Ordering::Release);
    }

    pub fn submit(
        &self,
        context: String,
        epoch: u64,
        batches: Vec<ScoreBatch>,
        generate: Option<String>,
    ) {
        self.latest.store(epoch, Ordering::Release);
        if self
            .jobs
            .send(Job {
                context,
                epoch,
                batches,
                generate,
            })
            .is_err()
        {
            tracing::warn!("神经重打分线程已退出");
        }
    }

    /// 取一条打好的分；没有就 `None`。
    pub fn poll(&self) -> Option<Scored> {
        self.results.try_recv().ok()
    }
}

impl Drop for RescoreWorker {
    fn drop(&mut self) {
        self.latest.store(0, Ordering::Release);
        // 关掉任务通道线程就会退出；已在算的单批不能中断，但不继续算剩下的批次。
        let _ = self.handle.take();
    }
}
