//! 真实产品词库下的逐字造词、重载与隐私检查；输入词只用于选择现有单字。

use std::path::Path;

use qingjian_core::{CandidateKind, CandidateView, Engine};
use qingjian_learning::FrequencyLearner;
use serde_json::{Value, json};

use crate::error::CliError;

fn select_character(engine: &mut Engine, text: char) -> Option<String> {
    engine.set_candidate_view(CandidateView::Characters);
    let candidate = engine
        .query()
        .ok()?
        .candidates
        .items
        .into_iter()
        .find(|c| c.kind == CandidateKind::Chinese && c.text == text.to_string())?;
    Some(engine.commit(&candidate))
}

fn spell(engine: &mut Engine, keys: &str, text: &str) -> bool {
    engine.set_input(keys);
    let mut output = String::new();
    for character in text.chars() {
        let Some(committed) = select_character(engine, character) else {
            return false;
        };
        output.push_str(&committed);
    }
    output == text && engine.composition().is_empty()
}

fn learned(engine: &Engine, text: &str, syllables: &[&str]) -> bool {
    engine.learner().user_words().is_some_and(|words| {
        words
            .lookup(syllables, false)
            .iter()
            .any(|w| w.text == text)
    })
}

pub(super) fn run(folder: &Path) -> Result<Vec<Value>, CliError> {
    let mut checks = Vec::new();
    for (id, text, syllables) in [
        ("word-two-characters", "珺玥", vec!["jun", "yue"]),
        (
            "word-three-characters",
            "珺玥青",
            vec!["jun", "yue", "qing"],
        ),
        (
            "word-four-characters",
            "珺玥青舟",
            vec!["jun", "yue", "qing", "zhou"],
        ),
    ] {
        let mut engine = super::engine()?;
        let keys = syllables.concat();
        let selected = spell(&mut engine, &keys, text);
        let saved = learned(&engine, text, &syllables);
        engine.break_chain();
        engine.set_input(&keys);
        let first = engine
            .query()
            .map_err(std::io::Error::other)?
            .candidates
            .items
            .first()
            .map(|c| c.text.clone());
        checks.push(json!({"id":id, "passed":selected && saved && first.as_deref() == Some(text), "after":first}));
    }
    let directory = folder.join("words");
    std::fs::create_dir(&directory)?;
    let path = directory.join("user.tsv");
    let mut engine = super::engine()?.with_learner(Box::new(FrequencyLearner::from_path(&path)?));
    let selected = spell(&mut engine, "jyqz", "珺玥青舟");
    engine.flush_learning();
    drop(engine);
    let mut engine = super::engine()?.with_learner(Box::new(FrequencyLearner::from_path(&path)?));
    let saved = learned(&engine, "珺玥青舟", &["jun", "yue", "qing", "zhou"]);
    engine.set_input("junyueqingzhou");
    let first = engine
        .query()
        .map_err(std::io::Error::other)?
        .candidates
        .items
        .first()
        .map(|c| c.text.clone());
    checks.push(json!({"id":"word-reload", "passed":selected && saved && first.as_deref() == Some("珺玥青舟"), "after":first}));
    for (id, private) in [("word-private", true), ("word-disabled", false)] {
        let mut engine = super::engine()?;
        if private {
            engine.set_private(true);
        } else {
            engine.set_learning(false);
        }
        let selected = spell(&mut engine, "junyue", "珺玥");
        engine.set_private(false);
        engine.set_learning(true);
        checks.push(
            json!({"id":id, "passed":selected && !learned(&engine,"珺玥", &["jun","yue"])
            && engine.learner().choice_weight("junyue","珺玥") == 0}),
        );
    }
    let mut engine = super::engine()?;
    engine.set_input("junyueqing");
    let held = select_character(&mut engine, '珺') == Some(String::new())
        && select_character(&mut engine, '玥') == Some(String::new());
    engine.discard_input();
    checks.push(
        json!({"id":"word-cancelled", "passed":held && engine.learner().user_words().is_none()}),
    );
    Ok(checks)
}
