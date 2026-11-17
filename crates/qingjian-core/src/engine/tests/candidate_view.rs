//! 展示限额不裁内部搜索；拆字选择必须正确消费拼音并能回到完整列表。

use crate::{CandidateView, Engine};
use qingjian_dictionary::Dictionary;

fn engine() -> Engine {
    let mut source = String::from("开\tkai\t99999\n凯\tkai\t5000\n发\tfa\t9000\n");
    for (i, c) in ('\u{4e00}'..='\u{4e23}').enumerate() {
        source.push_str(&format!("开{c}\tkai fa\t{}\n", 90000 - i * 100));
    }
    let mut engine = Engine::new(Dictionary::parse(&source).unwrap());
    engine.set_default_candidate_view(CandidateView::Compact);
    engine
}

#[test]
fn compact_limits_words_but_keeps_all_characters_and_more_restores_every_word() {
    let mut engine = engine();
    engine.set_input("kaifa");
    engine.set_candidate_view(CandidateView::All);
    let full = engine.query().unwrap().candidates.items;
    assert!(full.iter().filter(|c| c.text.chars().count() > 1).count() > 18);
    engine.set_candidate_view(CandidateView::Compact);
    let compact = engine.query().unwrap().candidates.items;
    assert_eq!(
        compact
            .iter()
            .filter(|c| c.text.chars().count() > 1)
            .count(),
        CandidateView::WORD_LIMIT
    );
    for single in full.iter().filter(|c| c.text.chars().count() == 1) {
        assert!(compact.contains(single), "{} disappeared", single.text);
    }
    assert_eq!(&full[..5], &compact[..5]);
    engine.set_candidate_view(CandidateView::All);
    assert_eq!(engine.query().unwrap().candidates.items, full);
}

#[test]
fn selecting_a_character_keeps_the_remaining_syllables_and_character_view() {
    let mut engine = engine();
    engine.set_hold_selections(false);
    engine.set_input("kaifa");
    engine.set_candidate_view(CandidateView::Characters);
    let items = engine.query().unwrap().candidates.items;
    assert!(!items.is_empty());
    assert!(items.iter().all(|c| c.text.chars().count() == 1));
    let chosen = items.iter().find(|c| c.text == "凯").unwrap();
    assert_eq!(engine.commit(chosen), "凯");
    assert_eq!(engine.composition().text(), "fa");
    assert_eq!(engine.candidate_view(), CandidateView::Characters);
    let fa = engine.query().unwrap().candidates.items[0].clone();
    assert_eq!(engine.commit(&fa), "发");
    engine.push('k');
    assert_eq!(engine.candidate_view(), CandidateView::Compact);
}

#[test]
fn new_composition_clear_and_privacy_boundaries_reset_the_view() {
    let mut engine = engine();
    for discard in [false, true] {
        engine.set_input("kaifa");
        engine.set_candidate_view(CandidateView::All);
        if discard {
            engine.discard_input();
        } else {
            engine.clear();
        }
        assert_eq!(engine.candidate_view(), CandidateView::Compact);
    }
    engine.set_input("kaifa");
    engine.set_candidate_view(CandidateView::Characters);
    engine.set_input("kai");
    assert_eq!(engine.candidate_view(), CandidateView::Compact);
}

#[test]
fn character_view_recovers_characters_lost_at_the_word_preselection_boundary() {
    let mut source = String::from("文\twen\t1\n");
    for c in '\u{4e00}'..='\u{5050}' {
        source.push_str(&format!("文{c}\twen jian\t10000\n"));
    }
    let mut engine = Engine::new(Dictionary::parse(&source).unwrap());
    engine.set_hold_selections(false);
    engine.set_input("wjfgw");
    engine.set_candidate_view(CandidateView::Characters);
    let items = engine.query().unwrap().candidates.items;
    let wen = items
        .iter()
        .find(|c| c.text == "文")
        .expect("first-syllable character must survive preselection");
    assert_eq!(engine.commit(wen), "文");
    assert_eq!(engine.composition().text(), "jfgw");
}

#[test]
fn character_view_puts_the_complete_first_reading_before_broad_prefix_readings() {
    let mut source = String::from("军\tjun\t9000\n珺\tjun\t1\n月\tyue\t9000\n");
    for c in '\u{4e00}'..='\u{4e70}' {
        source.push_str(&format!("{c}\tju\t10000\n"));
    }
    let mut engine = Engine::new(Dictionary::parse(&source).unwrap());
    engine.set_input("junyue");
    engine.set_candidate_view(CandidateView::Characters);
    let items = engine.query().unwrap().candidates.items;
    assert_eq!(items[0].text, "军");
    assert_eq!(
        items[1].text, "珺",
        "exact first reading must precede ju/j alternatives"
    );
}
