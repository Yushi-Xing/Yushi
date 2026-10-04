//! 与后台任务同代的模型结果。

use super::super::batch::ScoreBatch;

/// 后台算好的结果，与任务一一对应。
pub(crate) struct Scored {
    pub epoch: u64,

    pub batches: Vec<ScoreBatch>,

    /// 生成用的按键与生成出来的整句，对应任务里的 `generate`。
    pub generated: Option<(String, Vec<String>)>,
}
