//! 同一输入的选择学习：即时排序、落盘及模型重排。

mod selection_model;

use std::sync::Arc;
use std::time::{Duration, Instant};

use qingjian_core::{Engine, Learner};
use qingjian_dictionary::Dictionary;
use qingjian_learning::FrequencyLearner;

fn engine(learner: FrequencyLearner) -> Engine {
    Engine::new(
        Dictionary::parse("没\tmei\t10000\n买\tmai\t9000\n嘛\tma\t8000\n吗\tma\t100\n").unwrap(),
    )
    .with_learner(Box::new(learner))
}

fn choose_ma(engine: &mut Engine) {
    engine.set_input("m");
    let query = engine.query().unwrap();
    let chosen = query
        .candidates
        .items
        .iter()
        .find(|c| c.text == "吗")
        .unwrap();
    assert_eq!(engine.commit(chosen), "吗");
    engine.break_chain();
}

fn first(engine: &mut Engine) -> String {
    engine.set_input("m");
    engine.query().unwrap().candidates.items[0].text.clone()
}

#[test]
fn choosing_the_fourth_candidate_promotes_it_on_the_next_same_input() {
    let mut engine = engine(FrequencyLearner::default());
    engine.set_input("m");
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "没");
    assert_eq!(query.candidates.items[3].text, "吗");
    choose_ma(&mut engine);
    assert_eq!(engine.learner().choice_weight("m", "吗"), 1);
    assert_eq!(engine.learner().choice_weight("ma", "吗"), 0);
    assert_eq!(first(&mut engine), "吗");
}

#[test]
fn a_new_choice_overrides_existing_counts_for_the_same_typed_input() {
    let mut learner = FrequencyLearner::default();
    for _ in 0..100 {
        learner.record_choice("m", "mei", "没");
    }
    let mut engine = engine(learner);
    assert_eq!(first(&mut engine), "没");
    choose_ma(&mut engine);
    assert_eq!(first(&mut engine), "吗");
    assert_eq!(engine.learner().choice_weight("m", "没"), 100);
    assert_eq!(engine.learner().choice_weight("m", "吗"), 1);
}

#[test]
fn disabled_learning_and_private_input_do_not_record_the_selection() {
    for private in [false, true] {
        let mut engine = engine(FrequencyLearner::default());
        if private {
            engine.set_private(true);
        } else {
            engine.set_learning(false);
        }
        choose_ma(&mut engine);
        engine.set_private(false);
        engine.set_learning(true);
        assert_eq!(engine.learner().choice_weight("m", "吗"), 0);
        assert_eq!(first(&mut engine), "没");
    }
}

#[test]
fn learned_selection_survives_flushing_and_reopening() {
    let folder = std::env::temp_dir().join(format!(
        "qingjian-selection-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("user.tsv");
    let mut original = engine(FrequencyLearner::from_path(&path).unwrap());
    choose_ma(&mut original);
    original.flush_learning();
    let mut restored = engine(FrequencyLearner::from_path(&path).unwrap());
    assert_eq!(first(&mut restored), "吗");
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn choice_model_cannot_override_the_learned_preference_for_the_same_input() {
    let mut learner = FrequencyLearner::default();
    for _ in 0..100 {
        learner.record_choice("m", "mei", "没");
    }
    let mut engine = engine(learner);
    choose_ma(&mut engine);
    engine.set_choice_model(Some(Arc::new(selection_model::PrefersMei)));
    assert_eq!(first(&mut engine), "吗");
    let started = Instant::now();
    while engine.rescoring_pending() {
        assert!(engine.request_rescoring());
        while !engine.poll_rescoring() {
            assert!(started.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(5));
        }
        engine.query().unwrap();
    }
    assert_eq!(first(&mut engine), "吗");
}
