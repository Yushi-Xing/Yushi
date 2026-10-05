//! 长句漏键、简拼混输与末尾缩写的输入质量回归；不接模型或网络。

use crate::{CandidateKind, Engine};
use qingjian_dictionary::Dictionary;

#[test]
fn complete_dictionary_word_is_not_hidden_by_a_sentence_that_drops_the_last_initial() {
    let mut engine = Engine::new(Dictionary::parse("环太平洋\thuan tai ping yang\t3000\n太平\ttai ping\t100000\n还\thuan\t900000\n环\thuan\t1000\n").unwrap());
    engine.set_input("huantaipingy");
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "环太平洋");
    let candidate = query.candidates.items[0].clone();
    assert_eq!(engine.commit(&candidate), "环太平洋");
    assert!(engine.composition().is_empty());
}

#[test]
fn full_syllable_typo_can_be_recovered_between_abbreviated_syllables() {
    let mut engine = Engine::new(Dictionary::parse("你觉得\tni jue de\t200000\n这个\tzhe ge\t200000\n电影\tdian ying\t200000\n怎么样\tzen me yang\t200000\n嗲\tdia\t10\n又\tyou\t1000\n").unwrap());
    engine.set_input("nijuedezhegdiayzenmy");
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "你觉得这个电影怎么样");
    assert_eq!(
        engine.commit(&query.candidates.items[0]),
        "你觉得这个电影怎么样"
    );
    assert!(engine.composition().is_empty());
}

#[test]
fn a_missing_key_in_a_long_sentence_is_corrected_without_a_neural_model() {
    let mut engine = Engine::new(Dictionary::parse("我们今天\two men jin tian\t200000\n下午\txia wu\t200000\n一起\tyi qi\t200000\n讨论\ttao lun\t200000\n输入法\tshu ru fa\t200000\n问题\twen ti\t200000\n").unwrap());
    engine.set_input("womenjintianxiawuyiqitaolunshurufaweti");
    assert!(engine.composition().text().len() > 24);
    let query = engine.query().unwrap();
    assert!(query.correction.is_some());
    assert_eq!(
        query.candidates.items[0].text,
        "我们今天下午一起讨论输入法问题"
    );
    assert_eq!(
        engine.commit(&query.candidates.items[0]),
        "我们今天下午一起讨论输入法问题"
    );
    assert!(engine.composition().is_empty());
}

#[test]
fn exact_reading_is_not_replaced_by_a_more_common_prefix_completion() {
    let mut engine = Engine::new(
        Dictionary::parse("嗲声\tdia sheng\t5000\n电声\tdian sheng\t500000\n").unwrap(),
    );
    engine.set_input("diasheng");
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "嗲声");
    assert_eq!(query.candidates.items[0].kind, CandidateKind::Chinese);
    assert!(query.correction.is_none());
}

#[test]
fn exploring_long_sentence_corrections_does_not_queue_every_variant_for_the_model() {
    struct Scorer;
    impl crate::sentence::SentenceScorer for Scorer {
        fn score(&self, _context: &str, _keys: &str, texts: &[&str]) -> Vec<f64> {
            vec![-1.0; texts.len()]
        }
    }
    let mut engine = Engine::new(Dictionary::parse("我们今天\two men jin tian\t200000\n下午\txia wu\t200000\n一起\tyi qi\t200000\n讨论\ttao lun\t200000\n输入法\tshu ru fa\t200000\n问题\twen ti\t200000\n").unwrap()).with_async_sentence_scorer(Box::new(Scorer), None, None, None);
    engine.set_input("womenjintianxiawuyiqitaolunshurufaweti");
    assert!(engine.query().unwrap().correction.is_some());
    assert!(engine.neural_cache.borrow_mut().take_wanted().len() <= 1);
}
