//! 逐字造词走真实候选、组句、用户词落盘路径，覆盖首次学习及边界。

use qingjian_core::{CandidateKind, CandidateView, Engine};
use qingjian_dictionary::Dictionary;
use qingjian_learning::FrequencyLearner;

fn engine(learner: FrequencyLearner) -> Engine {
    Engine::new(
        Dictionary::parse(
            "珺\tjun\t100\n玥\tyue\t100\n青\tqing\t100\n舟\tzhou\t100\n桥\tqiao\t100\n",
        )
        .unwrap(),
    )
    .with_learner(Box::new(learner))
}

fn select(engine: &mut Engine, text: &str) -> String {
    engine.set_candidate_view(CandidateView::Characters);
    let candidate = engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .find(|c| c.text == text && c.kind == CandidateKind::Chinese)
        .unwrap_or_else(|| panic!("missing {text} for {}", engine.composition().text()));
    engine.commit(&candidate)
}

fn spell(engine: &mut Engine, keys: &str, text: &str) -> String {
    engine.set_input(keys);
    text.chars()
        .map(|c| select(engine, &c.to_string()))
        .collect()
}

fn learned(engine: &Engine, text: &str, syllables: &[&str]) -> bool {
    engine.learner().user_words().is_some_and(|words| {
        words
            .lookup(syllables, false)
            .iter()
            .any(|w| w.text == text)
    })
}

#[test]
fn completing_two_to_four_explicit_characters_learns_once_in_either_commit_mode() {
    for hold in [false, true] {
        for (text, syllables) in [
            ("珺玥", vec!["jun", "yue"]),
            ("珺玥青", vec!["jun", "yue", "qing"]),
            ("珺玥青舟", vec!["jun", "yue", "qing", "zhou"]),
        ] {
            let mut engine = engine(FrequencyLearner::default());
            engine.set_hold_selections(hold);
            let keys = syllables.concat();
            assert_eq!(spell(&mut engine, &keys, text), text);
            assert!(learned(&engine, text, &syllables), "{text}, hold={hold}");
            engine.break_chain();
            engine.set_input(&keys);
            assert_eq!(engine.query().unwrap().candidates.items[0].text, text);
        }
    }
}

#[test]
fn abbreviation_created_word_survives_reload_and_supports_full_pinyin() {
    let dir = std::env::temp_dir().join(format!(
        "qingjian-word-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("user.tsv");
    let mut original = engine(FrequencyLearner::from_path(&path).unwrap());
    assert_eq!(spell(&mut original, "jyqz", "珺玥青舟"), "珺玥青舟");
    original.flush_learning();
    drop(original);
    let mut restored = engine(FrequencyLearner::from_path(&path).unwrap());
    assert!(learned(
        &restored,
        "珺玥青舟",
        &["jun", "yue", "qing", "zhou"]
    ));
    for keys in ["jyqz", "junyueqingzhou"] {
        restored.set_input(keys);
        assert_eq!(
            restored.query().unwrap().candidates.items[0].text,
            "珺玥青舟"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn disabled_and_private_input_do_not_create_words() {
    for private in [false, true] {
        let mut engine = engine(FrequencyLearner::default());
        if private {
            engine.set_private(true);
        } else {
            engine.set_learning(false);
        }
        assert_eq!(spell(&mut engine, "junyue", "珺玥"), "珺玥");
        engine.set_private(false);
        engine.set_learning(true);
        assert!(!learned(&engine, "珺玥", &["jun", "yue"]));
        assert_eq!(engine.learner().choice_weight("junyue", "珺玥"), 0);
    }
}

#[test]
fn cancelled_incomplete_composition_does_not_create_a_word() {
    let mut engine = engine(FrequencyLearner::default());
    engine.set_input("junyueqing");
    assert!(select(&mut engine, "珺").is_empty());
    assert!(select(&mut engine, "玥").is_empty());
    engine.discard_input();
    assert!(engine.learner().user_words().is_none());
}

#[test]
fn separate_buffers_and_punctuation_do_not_immediately_join_words() {
    for separated in [false, true] {
        let mut engine = engine(FrequencyLearner::default());
        assert_eq!(spell(&mut engine, "jun", "珺"), "珺");
        if separated {
            engine.punctuate(',');
        }
        assert_eq!(spell(&mut engine, "yue", "玥"), "玥");
        assert!(engine.learner().user_words().is_none());
    }
}

#[test]
fn completed_long_compositions_are_remembered_within_twenty_characters() {
    let mut engine = engine(FrequencyLearner::default());
    assert_eq!(
        spell(&mut engine, "junyueqingzhouqiao", "珺玥青舟桥"),
        "珺玥青舟桥"
    );
    assert!(learned(
        &engine,
        "珺玥青舟桥",
        &["jun", "yue", "qing", "zhou", "qiao"]
    ));
    engine.set_input("jyqzq");
    assert_eq!(
        engine.query().unwrap().candidates.items[0].text,
        "珺玥青舟桥"
    );

    let text = "珺".repeat(21);
    assert_eq!(spell(&mut engine, &"jun".repeat(21), &text), text);
    assert!(!learned(&engine, &text, &["jun"; 21]));
}

#[test]
fn a_sentence_completed_after_an_explicit_selection_is_remembered() {
    let mut engine = engine(FrequencyLearner::default());
    engine.set_input("junyueqing");
    select(&mut engine, "珺");
    engine.set_candidate_view(CandidateView::All);
    let rest = engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .find(|c| c.kind == CandidateKind::Sentence && c.text == "玥青")
        .unwrap();
    assert_eq!(engine.commit(&rest), "珺玥青");
    assert!(learned(&engine, "珺玥青", &["jun", "yue", "qing"]));
    engine.set_input("jyq");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "珺玥青");
}
