//! 选词任务所属的输入与配置；同一输入的模型结果重查不另起一代。

use super::ChoiceTier;

#[derive(PartialEq, Eq)]
pub(super) struct Condition {
    pub(super) input: String,

    pub(super) context: String,

    pub(super) english: bool,

    pub(super) tier: ChoiceTier,
}
