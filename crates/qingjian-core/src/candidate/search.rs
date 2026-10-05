//! 搜索宿主的转换快照：排除预测、部分转换与重复前缀，保留引擎顺序。

use super::{Candidate, CandidateKind};
use crate::parser;

pub fn search_conversions(items: &[Candidate], keys: &str) -> Vec<String> {
    let syllables = parser::segment(keys)
        .ok()
        .and_then(|s| s.first().map(|s| s.syllables.len()));
    let mut texts = Vec::new();
    for candidate in items {
        let conversion = match candidate.kind {
            CandidateKind::Chinese | CandidateKind::Sentence => {
                syllables.is_some_and(|n| candidate.syllables.len() == n)
            }
            CandidateKind::English | CandidateKind::Code => true,
            _ => false,
        };
        if conversion && !candidate.text.is_empty() && !texts.contains(&candidate.text) {
            texts.push(candidate.text.clone());
        }
    }
    let all = texts.clone();
    texts.retain(|text| {
        !all.iter()
            .any(|other| other.len() > text.len() && other.starts_with(text))
    });
    texts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_exclude_predictions_partial_conversions_and_prefix_overlap() {
        let item = |text: &str, kind, count| Candidate {
            text: text.into(),
            kind,
            syllables: vec!["a".into(); count],
            reading: None,
            translation: None,
            aux_code: None,
        };
        let items = vec![
            item("你", CandidateKind::Chinese, 1),
            item("你好", CandidateKind::Chinese, 2),
            item("你好啊", CandidateKind::Cloud, 2),
            item("你好", CandidateKind::Sentence, 2),
            item("拟好", CandidateKind::Chinese, 2),
            item("你", CandidateKind::Chinese, 2),
            item("编造", CandidateKind::Generated, 2),
        ];
        assert_eq!(search_conversions(&items, "nihao"), ["你好", "拟好"]);
        assert!(search_conversions(&items, "").is_empty());
        assert!(search_conversions(&items, "bad input").is_empty());
    }
}
