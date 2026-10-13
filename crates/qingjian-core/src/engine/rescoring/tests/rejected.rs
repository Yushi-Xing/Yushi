//! 模型拒绝或失败的批次只尝试一次，不因候选轮询不断重试。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::{engine, path};
use crate::sentence::SentenceScorer;

struct Rejects(Arc<AtomicUsize>);

impl SentenceScorer for Rejects {
    fn score(&self, _context: &str, _keys: &str, _texts: &[&str]) -> Vec<f64> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }
}

#[test]
fn rejected_sync_batch_keeps_static_order_until_condition_changes() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut engine =
        engine().with_sentence_scorer(Box::new(Rejects(calls.clone())), None, None, None);
    for _ in 0..20 {
        let mut paths = vec![path("嗯", -10.0), path("恩", -11.0)];
        engine.rescore_paths(&mut paths, "en");
        assert_eq!(paths[0].text, "嗯");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    engine.set_rescoring_context(Some("新的前文".to_owned()));
    engine.rescore_paths(&mut [path("嗯", -10.0), path("恩", -11.0)], "en");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn rejected_async_batch_settles_and_new_reading_can_still_run() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut engine =
        engine().with_async_sentence_scorer(Box::new(Rejects(calls.clone())), None, None, None);
    for (index, keys) in ["en", "eng"].into_iter().enumerate() {
        engine.rescore_paths(&mut [path("嗯", -10.0), path("恩", -11.0)], keys);
        assert!(engine.request_rescoring());
        let started = Instant::now();
        while !engine.poll_rescoring() {
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
        for _ in 0..20 {
            let mut paths = vec![path("嗯", -10.0), path("恩", -11.0)];
            engine.rescore_paths(&mut paths, keys);
            assert_eq!(paths[0].text, "嗯");
            assert!(!engine.rescoring_pending());
            assert!(!engine.request_rescoring());
        }
        assert_eq!(calls.load(Ordering::SeqCst), index + 1);
    }
}
