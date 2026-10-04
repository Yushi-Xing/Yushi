//! 模型缓存容量、查询代隔离和读法分数不串用。

use crate::engine::rescoring::NeuralCache;

#[test]
fn different_readings_keep_independent_scores_and_context_resets_them() {
    let mut cache = NeuralCache::default();
    cache.prepare("前文", "scope");
    for (keys, score) in [("a", -1.0), ("b", -10.0)] {
        cache.ensure_condition("前文", keys);
        assert!(cache.reserve("同文"));
        cache.insert("同文", score);
    }
    for (keys, score) in [("a", -1.0), ("b", -10.0)] {
        cache.ensure_condition("前文", keys);
        assert_eq!(cache.get("同文"), Some(score));
    }
    let epoch = cache.epoch();
    cache.prepare("新前文", "scope");
    assert_ne!(cache.epoch(), epoch);
    assert_eq!(cache.get("同文"), None);
}

#[test]
fn full_cache_stays_bounded_without_clearing_completed_scores() {
    let mut cache = NeuralCache::default();
    cache.prepare("", "scope");
    for i in 0..1100 {
        cache.ensure_condition("", &i.to_string());
        cache.want("候选");
    }
    let wanted = cache.take_wanted();
    assert_eq!(wanted.len(), 1024);
    for (keys, texts) in wanted {
        cache.insert_for(&keys, &texts[0], -1.0);
    }
    for i in 0..1100 {
        cache.ensure_condition("", &i.to_string());
        if i < 1024 {
            assert_eq!(cache.get("候选"), Some(-1.0));
        }
        cache.want("候选");
    }
    assert!(!cache.has_wanted());
}

#[test]
fn switching_input_and_recreating_a_session_never_reuses_an_old_epoch() {
    let mut cache = NeuralCache::default();
    cache.prepare("", "first");
    let old = cache.epoch();
    cache.prepare("", "second");
    assert_ne!(cache.epoch(), old);
    let other = cache.epoch();
    cache = NeuralCache::default();
    cache.prepare("", "first");
    assert_ne!(cache.epoch(), old);
    assert_ne!(cache.epoch(), other);
}
