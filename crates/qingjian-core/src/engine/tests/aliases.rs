//! 全拼键盘别名的查词、显示和上屏回归。

use super::*;

#[test]
fn keyboard_v_aliases_preserve_lookup_preedit_and_commit() {
    let dictionary = Dictionary::parse("居住\tju zhu\t9000\n提取\tti qu\t8000\n序号\txu hao\t7000\n语言\tyu yan\t6000\n学习\txue xi\t5000\n").unwrap();
    let mut engine = Engine::new(dictionary);
    for (keys, canonical, expected) in [
        ("jvzhu", "juzhu", "居住"),
        ("tiqv", "tiqu", "提取"),
        ("xvhao", "xuhao", "序号"),
        ("yvyan", "yuyan", "语言"),
    ] {
        engine.set_input(canonical);
        let canonical_items = engine.query().unwrap().candidates.items;
        engine.set_input(keys);
        let query = engine.query().unwrap();
        assert!(query.correction.is_none(), "{keys}");
        assert_eq!(query.tail, "", "{keys}");
        assert_eq!(query.candidates.items[0].text, expected, "{keys}");
        assert_eq!(
            query.candidates.items[0].syllables,
            canonical_items[0].syllables
        );
        assert!(query.marked_text().contains('v'), "{keys}");
        let candidate = query.candidates.items[0].clone();
        engine.commit(&candidate);
        assert!(engine.composition().is_empty(), "{keys}");
    }
    engine.set_input("xuexitiqv");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "学习提取");
}
