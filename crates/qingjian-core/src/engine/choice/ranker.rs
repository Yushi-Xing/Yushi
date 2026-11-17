//! 接上的选词模型与它的缓存。

use std::cell::RefCell;
use std::sync::Arc;

use super::condition::Condition;
use super::job::Job;
use super::worker::ChoiceWorker;
use super::{CharTable, ChoiceModel, ChoiceRequest, ComposeRequest};
use crate::engine::rescoring::WakeSlot;

/// 字分布最多缓存几张：打平的切分一次查询要比两三种，回退删字时前一段输入的表还用得上。
const TABLES: usize = 8;

/// 接上的选词模型：同步对齐用的模型、后台线程，和最近一页的分。
pub(crate) struct ChoiceRanker {
    pub model: Arc<dyn ChoiceModel>,
    pub worker: ChoiceWorker,

    condition: RefCell<Option<Condition>>,

    /// 同一输入尚未收回的任务，重复查询不重新提交。
    pub(super) in_flight: RefCell<Option<Job>>,

    /// 最近打好分的一页；候选窗里一次只有一页要排，留一条就够。
    pub scored: RefCell<Option<(ChoiceRequest, Vec<f32>, u64)>>,

    /// 查询时还没分、等壳在停顿后送去后台的那页。
    pub wanted: RefCell<Option<ChoiceRequest>>,

    /// 算好的字分布，新的在后；`None` 是模型出不了，也记下免得再算。
    tables: RefCell<Vec<(String, Option<Arc<CharTable>>)>>,

    /// 查询时还没有字分布、等壳送去后台的输入。
    pub compose_wanted: RefCell<Vec<ComposeRequest>>,
}

impl ChoiceRanker {
    pub fn new(model: Arc<dyn ChoiceModel>, wake: WakeSlot) -> Self {
        Self {
            worker: ChoiceWorker::spawn(model.clone(), wake),
            condition: RefCell::new(None),
            in_flight: RefCell::new(None),
            model,
            scored: RefCell::new(None),
            wanted: RefCell::new(None),
            tables: RefCell::new(Vec::new()),
            compose_wanted: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn prepare(&self, condition: Condition) {
        let mut previous = self.condition.borrow_mut();
        if previous.as_ref() != Some(&condition) {
            self.worker.cancel();
            self.in_flight.borrow_mut().take();
            *previous = Some(condition);
        }
        self.wanted.borrow_mut().take();
        self.compose_wanted.borrow_mut().clear();
    }

    pub(super) fn cancel(&self) {
        self.worker.cancel();
        self.in_flight.borrow_mut().take();
        self.condition.borrow_mut().take();
        self.wanted.borrow_mut().take();
        self.compose_wanted.borrow_mut().clear();
    }

    pub(super) fn submit(&self, job: Job) {
        let mut in_flight = self.in_flight.borrow_mut();
        if in_flight.as_ref() != Some(&job) {
            self.worker.submit(job.clone());
            *in_flight = Some(job);
        }
    }

    /// 这页的分与打分花的毫秒数。
    pub fn cached(&self, request: &ChoiceRequest) -> Option<(Vec<f32>, u64)> {
        self.scored
            .borrow()
            .as_ref()
            .filter(|(scored, _, _)| scored == request)
            .map(|(_, logits, ms)| (logits.clone(), *ms))
    }

    /// 这段输入的字分布：算过就给（外层 `None` 是还没算）。
    pub fn table(&self, key: &str) -> Option<Option<Arc<CharTable>>> {
        self.tables
            .borrow()
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, table)| table.clone())
    }

    pub fn insert_table(&self, key: String, table: Option<Arc<CharTable>>) {
        let mut tables = self.tables.borrow_mut();
        tables.retain(|(k, _)| *k != key);
        tables.push((key, table));
        if tables.len() > TABLES {
            tables.remove(0);
        }
    }
}
