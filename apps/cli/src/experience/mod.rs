//! 离线打字体验回放：逐键查询、实际选词和学习检查，不读取个人配置或联网。

mod interactions;
mod scenario;
mod stability;
mod word_learning;

use std::path::Path;
use std::time::Instant;

use qingjian_core::{Candidate, CandidateKind, CandidateView, Engine};
use qingjian_dictionary::{Dictionary, WordList};
use qingjian_learning::FrequencyLearner;
use qingjian_lm::BigramModel;
use qingjian_platform::{DictionariesConfig, extra_dictionaries};
use serde_json::{Value, json};

use crate::error::CliError;
use scenario::Scenario;

/// 与产品的静态回退路径相同；每个任务新建学习器，避免前一道题教会下一道题。
fn engine() -> Result<Engine, CliError> {
    let dictionary = Dictionary::from_path("data/generated/dict.qj")?;
    let mut engine = Engine::new(dictionary)
        .with_learner(Box::<FrequencyLearner>::default())
        .with_language_model(Box::new(BigramModel::from_path(Path::new(
            "data/generated/lm.qj",
        ))?))
        .with_english(WordList::from_path("data/generated/english.qj")?);
    let mut dictionaries = extra_dictionaries::load(
        Some(Path::new("data/generated/dicts")),
        &[],
        &DictionariesConfig::default(),
    );
    dictionaries.push(Dictionary::builtin_patch()?);
    engine.set_extra_dictionaries(dictionaries);
    engine.set_default_candidate_view(CandidateView::Compact);
    Ok(engine)
}

fn query(engine: &Engine, timings: &mut Vec<f64>, errors: &mut Vec<String>) -> Vec<Candidate> {
    let started = Instant::now();
    let result = engine.query();
    timings.push(started.elapsed().as_secs_f64() * 1000.0);
    match result {
        Ok(query) => query.candidates.items,
        Err(error) => {
            errors.push(error.to_string());
            Vec::new()
        }
    }
}

fn texts(items: &[Candidate]) -> Vec<&str> {
    items.iter().map(|c| c.text.as_str()).collect()
}

/// 模拟人在候选中找目标的最长前缀；只能选择引擎实际给出的项。
fn choose(items: &[Candidate], remaining: &str, limit: usize) -> Option<usize> {
    items
        .iter()
        .take(limit)
        .enumerate()
        .filter(|(_, c)| !c.text.is_empty() && remaining.starts_with(&c.text))
        .filter(|(_, c)| c.kind != CandidateKind::Generated || c.text == remaining)
        .max_by_key(|(index, c)| (c.text.chars().count(), usize::MAX - index))
        .map(|(index, _)| index)
}

fn run_case(engine: &mut Engine, case: &Scenario, all: bool, word_limit: usize) -> Value {
    let mut result = run_attempt(engine, case, all, word_limit);
    if result["completed"] == true || result["output"] != "" {
        return result;
    }
    let Some(keys) = &case.retry_keys else {
        return result;
    };
    let started = Instant::now();
    engine.discard_input();
    let cancelled_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut retry_case = case.clone();
    retry_case.keys.clone_from(keys);
    retry_case.keystrokes = None;
    retry_case.retry_keys = None;
    retry_case.erase = 0;
    let retry = run_attempt(engine, &retry_case, all, word_limit);
    for field in ["actions", "page_turns", "view_changes"] {
        result[field] = json!(
            result[field].as_u64().unwrap()
                + retry[field].as_u64().unwrap()
                + u64::from(field == "actions")
        );
    }
    result["event_ms"]
        .as_array_mut()
        .unwrap()
        .push(json!(cancelled_ms));
    for field in ["event_ms", "query_ms", "query_errors"] {
        result[field]
            .as_array_mut()
            .unwrap()
            .extend(retry[field].as_array().unwrap().iter().cloned());
    }
    for field in ["output", "completed", "remaining_keys"] {
        result[field] = retry[field].clone();
    }
    result["stable"] = json!(result["stable"] == true && retry["stable"] == true);
    result["retry"] = retry;
    result
}

fn run_attempt(engine: &mut Engine, case: &Scenario, all: bool, word_limit: usize) -> Value {
    engine.set_candidate_word_limit(word_limit);
    engine.set_english_mode(case.english);
    let mut timings = Vec::new();
    let mut repeated_timings = Vec::new();
    let mut event_timings = Vec::new();
    let mut errors = Vec::new();
    let mut prefixes = Vec::new();
    let mut stable = true;
    let mut candidates = Vec::new();
    let keystrokes = case
        .keystrokes
        .clone()
        .unwrap_or_else(|| format!("{}{}", case.keys, "\u{0008}".repeat(case.erase)));
    for c in keystrokes.chars() {
        let started = Instant::now();
        if c == '\u{0008}' {
            engine.backspace();
        } else {
            engine.push(c);
        }
        if all {
            engine.set_candidate_view(CandidateView::All);
        }
        candidates = query(engine, &mut timings, &mut errors);
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        event_timings.push(elapsed);
        let repeated = query(engine, &mut repeated_timings, &mut errors);
        stable &= stability::same(engine.composition().text(), &candidates, &repeated);
        let refreshed: Vec<Value> = candidates
            .iter()
            .zip(&repeated)
            .enumerate()
            .filter(|(_, (before, after))| before.text != after.text)
            .map(|(index, (before, after))| {
                json!({"index":index, "before":before.text, "after":after.text,
                    "before_kind":before.kind, "after_kind":after.kind})
            })
            .collect();
        prefixes.push(json!({"key":c, "keys": engine.composition().text(),
            "top": candidates.first().map(|c| &c.text), "event_ms":elapsed, "refreshed":refreshed}));
    }
    let expected = case.text.trim_end_matches(['，', '？']);
    let cold = texts(&candidates)
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let rank = cold.iter().position(|text| text == expected);
    let mut output = String::new();
    let mut selections = Vec::new();
    let mut page_turns = 0;
    let mut view_changes = 0;
    // 有限步骤避免候选消费异常造成死循环。一次任务最多选择每个目标字符一次。
    for _ in 0..case.text.chars().count() {
        if engine.composition().is_empty() {
            break;
        }
        let selected = output.clone() + &engine.held_text();
        let Some(remaining) = expected.strip_prefix(&selected) else {
            break;
        };
        let mut index = choose(
            &candidates,
            remaining,
            if all { usize::MAX } else { word_limit },
        );
        if index.is_none() && !all {
            let started = Instant::now();
            engine.set_candidate_view(CandidateView::Characters);
            view_changes += 1;
            candidates = query(engine, &mut timings, &mut errors);
            event_timings.push(started.elapsed().as_secs_f64() * 1000.0);
            // 简拼有全拼补救路线时，单字也只看三页，模拟用户放弃继续翻找。
            index = choose(
                &candidates,
                remaining,
                if case.retry_keys.is_some() {
                    27
                } else {
                    usize::MAX
                },
            );
        }
        let Some(index) = index else {
            // 只有原始英文确实等于目标剩余部分时，才执行真正的原样上屏。
            if remaining.is_ascii() && engine.composition().text() == remaining {
                let started = Instant::now();
                output.push_str(&engine.take_raw());
                event_timings.push(started.elapsed().as_secs_f64() * 1000.0);
                selections.push(json!({"action":"enter", "text":remaining}));
            }
            break;
        };
        let candidate = candidates[index].clone();
        let before = engine.composition().text().to_owned();
        page_turns += index / 9;
        let started = Instant::now();
        let committed = engine.commit(&candidate);
        selections
            .push(json!({"action":"select", "index":index, "text":candidate.text, "keys":before}));
        output.push_str(&committed);
        if engine.composition().text() == before {
            break;
        }
        candidates = if engine.composition().is_empty() {
            Vec::new()
        } else {
            query(engine, &mut timings, &mut errors)
        };
        event_timings.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    if output == expected
        && let Some(mark) = case.punctuation
    {
        if let Some(converted) = engine.punctuate(mark) {
            output.push_str(converted);
        } else {
            engine.note_passthrough(mark);
            output.push(mark);
        }
    }
    let completed = output == case.text && engine.composition().is_empty();
    let actions = keystrokes.chars().count()
        + selections.len()
        + page_turns
        + 2 * view_changes
        + usize::from(case.punctuation.is_some());
    json!({
        "id":case.id, "scene":case.scene, "text":case.text, "keys":case.keys,
        "position":rank, "top":cold.first(), "candidates":cold,
        "prefixes":prefixes, "stable":stable, "query_errors":errors,
        "query_ms":timings, "repeat_query_ms":repeated_timings, "event_ms":event_timings,
        "interval_ms":case.interval_ms, "session":case.session, "keystrokes":keystrokes,
        "completed":completed, "output":output,
        "page_turns":page_turns, "view_changes":view_changes,
        "actions":actions, "selections":selections, "remaining_keys":engine.composition().text(),
    })
}

pub fn run(output: &Path, all: bool, word_limit: usize) -> Result<(), CliError> {
    let mut scenarios: Vec<Scenario> = serde_json::from_str(include_str!(
        "../../../../assets/eval/experience/scenarios.json"
    ))
    .map_err(std::io::Error::other)?;
    scenarios.extend(
        serde_json::from_str::<Vec<Scenario>>(include_str!(
            "../../../../assets/eval/experience/extended.json"
        ))
        .map_err(std::io::Error::other)?,
    );
    let mut results = Vec::new();
    let mut current = engine()?;
    let mut session = None;
    for case in &scenarios {
        if case.session.is_none() || case.session != session {
            current = engine()?;
        }
        session.clone_from(&case.session);
        results.push(run_case(&mut current, case, all, word_limit));
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let checks = interactions::run(&parent.join("learning-sandbox"))?;
    let report = json!({"schema":2, "mode":if all {"all"} else {"compact"}, "word_limit":word_limit,
        "results":results, "checks":checks, "network":false,
        "model":"static product fallback; neural evaluation is a separate release gate"});
    std::fs::write(
        output,
        serde_json::to_vec_pretty(&report).map_err(std::io::Error::other)?,
    )?;
    println!(
        "Experience replay saved: {} ({} scenarios)",
        output.display(),
        scenarios.len()
    );
    Ok(())
}
