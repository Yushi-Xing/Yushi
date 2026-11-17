//! 连续上屏后的整句候选应沿用同一条词转移上下文，失焦断链后恢复句首评分。

use crate::sentence::LanguageModel;
use crate::{CandidateKind, Engine};
use qingjian_dictionary::Dictionary;

struct ContextModel;

impl LanguageModel for ContextModel {
    fn log_prob(&self, previous: Option<&str>, word: &str) -> Option<f64> {
        Some(match (previous, word) {
            (Some("需要"), "办理") => -0.1,
            (Some("需要"), "班里") => -9.0,
            (_, "班里") => -1.0,
            (_, "办理") => -5.0,
            _ => -1.0,
        })
    }
}

#[test]
fn sentence_continues_the_committed_context_until_the_chain_breaks() {
    let dictionary = Dictionary::parse(
        "需要\txu yao\t1000\n班里\tban li\t1000\n办理\tban li\t100\n手续\tshou xu\t1000\n",
    )
    .unwrap();
    let mut engine = Engine::new(dictionary).with_language_model(Box::new(ContextModel));
    engine.set_input("xuyao");
    let needed = engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .find(|candidate| candidate.text == "需要" && candidate.kind == CandidateKind::Chinese)
        .unwrap();
    engine.commit(&needed);
    engine.set_input("banlishouxu");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "办理手续");
    engine.break_chain();
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "班里手续");
}
