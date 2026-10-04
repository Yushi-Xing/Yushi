//! 多读法共享缓存、实际混输查询及异步任务的停键收敛回归。

use super::{engine, path};
use crate::Engine;
use crate::sentence::SentenceScorer;
use qingjian_dictionary::{Dictionary, WordList};
use std::time::{Duration, Instant};

/// 同一输入的两种读法轮流查询，分数不能互相清空而形成停键后的计算循环。
#[test]
fn unchanged_alternative_readings_do_not_repeatedly_invoke_the_model() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Counts(Arc<AtomicUsize>);
    impl SentenceScorer for Counts {
        fn score(&self, _context: &str, _keys: &str, texts: &[&str]) -> Vec<f64> {
            self.0.fetch_add(1, Ordering::SeqCst);
            vec![-1.0; texts.len()]
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = engine().with_sentence_scorer(Box::new(Counts(calls.clone())), None, None, None);
    for _ in 0..20 {
        for keys in ["kaifang", "kaifan"] {
            let mut paths = vec![path("开饭", -10.0), path("开放", -11.0)];
            engine.rescore_paths(&mut paths, keys);
        }
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "停键后的相同读法应复用各自分数"
    );
}

fn wait(engine: &mut Engine) {
    let started = Instant::now();
    while !engine.poll_rescoring() {
        assert!(started.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn asynchronous_readings_are_scored_together_and_then_settle() {
    let mut engine =
        engine().with_async_sentence_scorer(Box::new(super::Prefers("开放")), None, None, None);
    for keys in ["kaifang", "kaifan"] {
        engine.rescore_paths(&mut [path("开饭", -10.0), path("开放", -11.0)], keys);
    }
    assert!(engine.request_rescoring());
    wait(&mut engine);
    for _ in 0..20 {
        for keys in ["kaifang", "kaifan"] {
            let mut paths = vec![path("开饭", -10.0), path("开放", -11.0)];
            engine.rescore_paths(&mut paths, keys);
            assert_eq!(paths[0].text, "开放");
        }
        assert!(!engine.rescoring_pending());
        assert!(!engine.request_rescoring());
    }
}

#[test]
fn mixed_english_query_stops_submitting_work_after_results_arrive() {
    let dictionary = Dictionary::parse("我\two\t900000\n的\tde\t800000\n我的\two de\t500000\n大\tda\t50000\n塔\tta\t3000\n巴\tba\t3000\n瑟\tse\t500\n").unwrap();
    let words = WordList::parse("database\tdatabase\t4310\n").unwrap();
    let mut engine = Engine::new(dictionary)
        .with_english(words)
        .with_async_sentence_scorer(Box::new(super::Prefers("我的")), None, None, None);
    engine.set_input("wodedatabase");
    let mut settled = false;
    for _ in 0..4 {
        let query = engine.query().unwrap();
        assert_eq!(query.candidates.items[0].text, "我的database");
        if !engine.request_rescoring() {
            settled = true;
            break;
        }
        wait(&mut engine);
    }
    assert!(settled, "停键后的实际混输查询不能自行不断重启模型");
    for _ in 0..20 {
        engine.query().unwrap();
        assert!(!engine.request_rescoring());
    }
}
