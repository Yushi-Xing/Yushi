//! 同一读法的一批候选及其后台模型分数。

pub(crate) struct ScoreBatch {
    pub keys: String,

    pub texts: Vec<String>,

    pub scores: Vec<f64>,
}
