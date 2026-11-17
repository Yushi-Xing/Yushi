//! 强烈偏爱「没」的模型，用来检查学习结果会不会被后续重排覆盖。

use qingjian_core::ChoiceModel;

pub struct PrefersMei;

impl ChoiceModel for PrefersMei {
    fn align(&self, _: &str, _: &[String]) -> Option<Vec<u32>> {
        Some(vec![1])
    }
    fn letter_units(&self, _: &str) -> Vec<u32> {
        vec![1]
    }
    fn score(&self, _: &str, _: &[u32], texts: &[&str]) -> Vec<f32> {
        texts
            .iter()
            .map(|t| if *t == "没" { 100.0 } else { 0.0 })
            .collect()
    }
}
