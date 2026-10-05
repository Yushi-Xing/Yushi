//! 模型误把漏键拼音抄成英文时，不挤掉已覆盖整段输入的中文候选。

use crate::sentence::SentenceScorer;
use crate::{CandidateKind, Engine};
use qingjian_dictionary::{Dictionary, WordList};

struct Generator(&'static str);

impl SentenceScorer for Generator {
    fn score(&self, _context: &str, _keys: &str, texts: &[&str]) -> Vec<f64> {
        vec![0.0; texts.len()]
    }

    fn generate(&self, _keys: &str, _beam: usize, _max_chars: usize) -> Vec<String> {
        vec![self.0.to_owned()]
    }
}

fn engine(generated: &'static str) -> Engine {
    Engine::new(Dictionary::parse("你觉得\tni jue de\t200000\n这个\tzhe ge\t200000\n电影\tdian ying\t200000\n怎么样\tzen me yang\t200000\n嗲\tdia\t10\n又\tyou\t1000\n").unwrap())
        .with_sentence_scorer(Box::new(Generator(generated)), Some(0.0), None, None)
}

#[test]
fn invented_english_does_not_override_a_complete_chinese_correction() {
    let mut engine = engine("你觉得这个Diay怎么样");
    engine.set_input("nijuedezhegdiayzenmy");
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "你觉得这个电影怎么样");
    assert!(
        query
            .candidates
            .items
            .iter()
            .skip(1)
            .any(|c| c.kind == CandidateKind::Generated && c.text == "你觉得这个Diay怎么样")
    );
}

#[test]
fn explicitly_typed_known_english_keeps_generation_priority() {
    let mut engine =
        engine("你觉得这个Diay怎么样").with_english(WordList::parse("Diay\tdiay\t1\n").unwrap());
    engine.set_input("nijuedezhegdiayzenmy");
    assert_eq!(
        engine.query().unwrap().candidates.items[0].text,
        "你觉得这个Diay怎么样"
    );
}

#[test]
fn chinese_generation_keeps_its_existing_priority() {
    let mut engine = engine("你觉得这部电影怎么样");
    engine.set_input("nijuedezhegdiayzenmy");
    assert_eq!(
        engine.query().unwrap().candidates.items[0].text,
        "你觉得这部电影怎么样"
    );
}

#[test]
fn unknown_english_is_kept_when_no_complete_chinese_path_exists() {
    let mut engine = Engine::new(Dictionary::parse("我用\two yong\t20000\n").unwrap())
        .with_sentence_scorer(Box::new(Generator("我用Zorbix开发")), Some(0.0), None, None);
    engine.set_input("woyongzorbixkaifa");
    assert_eq!(
        engine.query().unwrap().candidates.items[0].text,
        "我用Zorbix开发"
    );
}

#[test]
fn known_english_that_was_not_typed_does_not_override_chinese() {
    let mut engine = engine("你觉得这个Docker怎么样")
        .with_english(WordList::parse("Docker\tdocker\t1\n").unwrap());
    engine.set_input("nijuedezhegdiayzenmy");
    assert_eq!(
        engine.query().unwrap().candidates.items[0].text,
        "你觉得这个电影怎么样"
    );
}

#[test]
fn every_generated_english_token_needs_confirmation() {
    let mut engine = engine("你觉得这个Diay Foo怎么样")
        .with_english(WordList::parse("Diay\tdiay\t1\n").unwrap());
    engine.set_input("nijuedezhegdiayzenmy");
    assert_eq!(
        engine.query().unwrap().candidates.items[0].text,
        "你觉得这个电影怎么样"
    );
}
