//! 整句实验用的 P2C 打分器：与产品端 `qingjian_neural::P2cScorer` 同一条件，只是推理出错时直接终止。

use qingjian_core::sentence::SentenceScorer;
use qingjian_neural::CharScorer;

pub struct P2cScorer(pub CharScorer);

impl SentenceScorer for P2cScorer {
    fn score(&self, _context: &str, keys: &str, texts: &[&str]) -> Vec<f64> {
        // 与产品适配器使用同一字表外回退，实际推理错误仍使实验失败。
        if texts.iter().any(|text| !self.0.vocab().knows(text)) {
            return Vec::new();
        }
        // 实验不能静默回退到基线：任何推理错误立即终止，让不完整结果无法冒充成功。
        self.0
            .score_p2c(keys, texts)
            .expect("P2C evaluation scoring failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires verified shipped P2C model"]
    fn shipped_model_evaluation_matches_product_unknown_character_policy() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let scorer = CharScorer::load(
            &root.join("data/models/hanzhang-tongbian/hanzhang-tongbian-small.qjm"),
        )
        .unwrap();
        let unknown = ["嗯", "𰻞", "🙂"]
            .into_iter()
            .find(|text| !scorer.vocab().knows(text))
            .expect("negative control outside shipped vocabulary");
        assert!(scorer.vocab().knows("你好"));
        let scorer = P2cScorer(scorer);
        assert!(scorer.score("", "nihao", &[unknown, "你好"]).is_empty());
        assert!(scorer.score("", "nihao", &["你好", unknown]).is_empty());
        let scores = scorer.score("", "nihao", &["你好", "你"]);
        assert_eq!(scores.len(), 2);
        assert!(scores.iter().all(|score| score.is_finite()));
    }
}
