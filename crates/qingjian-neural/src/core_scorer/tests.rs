//! 未登录字符不得把某条合法路径的概率压到其他路径之下。

use std::path::Path;

use qingjian_core::sentence::SentenceScorer;

use crate::{CharScorer, P2cScorer, Vocab};

fn scorer() -> CharScorer {
    let vocab = Vocab::from_json(
        r#"{"tokens":["<pad>","<unk>","<eos>","<sep>","你","好","恩","n","i","h","a","o"]}"#,
        Path::new("synthetic-vocab.json"),
    )
    .unwrap();
    CharScorer::test_fixture(vocab)
}

fn verify_guard(model: &dyn SentenceScorer) {
    for candidates in [
        vec!["嗯", "恩"],
        vec!["恩", "嗯"],
        vec!["你好", "你嗯好"],
        vec!["𰻞", "恩"],
        vec!["🙂", "你好"],
    ] {
        assert!(
            model.score("你好", "nihao", &candidates).is_empty(),
            "{candidates:?}"
        );
    }
    let scores = model.score("你好", "nihao", &["你好", "恩"]);
    assert_eq!(scores.len(), 2);
    assert!(scores.iter().all(|score| score.is_finite()));
    assert!(model.score("你好", "nihao", &[]).is_empty());
}

#[test]
fn character_and_p2c_models_skip_unknown_candidate_batches() {
    verify_guard(&scorer());
    verify_guard(&P2cScorer::new(scorer()).unwrap());
}
