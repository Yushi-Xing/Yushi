//! 评测回归：错拼与上下文不丢样本，失败计入分母，首选不借答案挑选。

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;

use super::pair::Pair;
use super::{Report, collect, evaluate};

#[test]
fn identical_text_with_distinct_typo_or_context_remains_separate() {
    let path = std::env::temp_dir().join(format!("qingjian-eval-{}.tsv", std::process::id()));
    std::fs::write(
        &path,
        "你好\tnihao\t今天\n你好\tnihoa\t今天\n你好\tnihao\t昨天\n你好\tnihao\t今天\n",
    )
    .unwrap();
    let engine = Engine::new(Dictionary::parse("你好\tni hao\t100\n").unwrap());
    let mut report = Report::default();
    let result = collect(&engine, std::slice::from_ref(&path), &mut report);
    std::fs::remove_file(path).unwrap();
    let pairs = result.unwrap();
    assert_eq!(pairs.len(), 3);
    assert_eq!(pairs[1].pinyin, "nihoa");
    assert_eq!(pairs[2].context, "昨天");
}

#[test]
fn unparseable_queries_reduce_sentence_and_character_accuracy() {
    let mut engine = Engine::new(Dictionary::parse("你好\tni hao\t100\n").unwrap());
    let mut report = Report::default();
    for pinyin in ["nihao", "iiii"] {
        evaluate(
            &mut engine,
            &Pair {
                text: "你好".into(),
                pinyin: pinyin.into(),
                context: String::new(),
            },
            &mut report,
            3,
        );
    }
    assert_eq!((report.total, report.unparsable, report.top1), (2, 1, 1));
    assert_eq!(
        (report.chars_total, report.chars_correct, report.char_errors),
        (4, 2, 2)
    );
    assert!(report.to_string().contains("50.0%"));
    assert!(!report.to_string().contains("100.0%"));
}

#[test]
fn wrong_length_top_candidate_is_counted_without_looking_at_gold_length() {
    let mut engine =
        Engine::new(Dictionary::parse("开发者\tkai fa zhe\t900000\n开发\tkai fa\t100\n").unwrap());
    let mut report = Report::default();
    let row = evaluate(
        &mut engine,
        &Pair {
            text: "开发者".into(),
            pinyin: "kaifa".into(),
            context: String::new(),
        },
        &mut report,
        3,
    );
    let top = row["top"].as_str().unwrap();
    assert_eq!(
        report.char_errors,
        super::metrics::edit_distance("开发者", top)
    );
    assert_eq!(
        report.chars_correct,
        3usize.saturating_sub(report.char_errors)
    );
    assert_eq!(row["char_errors"], report.char_errors);
}
