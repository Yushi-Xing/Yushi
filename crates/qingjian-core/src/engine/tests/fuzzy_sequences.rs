//! 模糊音的完整输入、连续编辑、上屏消耗和配置切换回归。

use crate::{Engine, FuzzyRules};
use qingjian_dictionary::Dictionary;

fn engine() -> Engine {
    Engine::new(Dictionary::parse("听我说\tting wo shuo\t900000\n听说\tting shuo\t800000\n双手\tshuang shou\t700000\n明天\tming tian\t800000\n工程\tgong cheng\t800000\n程式\tcheng shi\t800000\n请我说\tqing wo shuo\t1000000\n听\tting\t90000\n我\two\t90000\n说\tshuo\t90000\n").unwrap())
}

#[test]
fn configured_nonstandard_fuzzy_syllables_beat_spelling_correction() {
    let mut engine = engine();
    engine.set_fuzzy(FuzzyRules::ALL);
    for (typed, expected) in [
        ("tinwosuo", "听我说"),
        ("tinsuo", "听说"),
        ("suangsou", "双手"),
        ("mintian", "明天"),
        ("gongcen", "工程"),
        ("censi", "程式"),
    ] {
        engine.clear();
        engine.set_input(typed);
        let query = engine.query().unwrap();
        assert!(query.correction.is_none(), "{typed}");
        let candidate = query
            .candidates
            .items
            .iter()
            .find(|c| c.text == expected)
            .unwrap_or_else(|| panic!("{typed}: {:?}", query.candidates.items));
        assert_eq!(engine.commit(&candidate.clone()), expected);
        assert!(engine.composition().is_empty(), "{typed}");
    }
}

#[test]
fn fuzzy_input_survives_each_prefix_backspace_and_retyping() {
    let mut engine = engine();
    engine.set_fuzzy(FuzzyRules {
        in_ing: true,
        s_sh: true,
        ..FuzzyRules::default()
    });
    for c in "tinwosuo".chars() {
        engine.push(c);
        engine.query().unwrap();
    }
    for _ in 0..3 {
        engine.backspace();
        engine.query().unwrap();
    }
    for c in "suo".chars() {
        engine.push(c);
        engine.query().unwrap();
    }
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "听我说");
    assert!(query.correction.is_none());
    assert_eq!(engine.take_raw(), "tinwosuo");
    assert!(engine.composition().is_empty());
}

#[test]
fn disabled_rules_and_apostrophes_do_not_accept_unrelated_nonstandard_readings() {
    let mut engine = engine();
    for (rules, typed, expected) in [
        (FuzzyRules::default(), "tinwosuo", false),
        (
            FuzzyRules {
                in_ing: true,
                ..FuzzyRules::default()
            },
            "tin'wo'shuo",
            true,
        ),
        (
            FuzzyRules {
                s_sh: true,
                ..FuzzyRules::default()
            },
            "tin'wo'shuo",
            false,
        ),
        (FuzzyRules::ALL, "tin'wo'suo", true),
    ] {
        engine.set_fuzzy(rules);
        engine.set_input(typed);
        let query = engine.query().unwrap();
        assert_eq!(
            query.candidates.items.iter().any(|c| c.text == "听我说"),
            expected,
            "{typed}: {rules:?}"
        );
    }
}
