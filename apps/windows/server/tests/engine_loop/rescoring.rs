//! 本地模型重排与隐私会话。

use crate::support::*;

#[test]
fn local_model_rescoring_reorders_sentence_after_pause() {
    // k 优路径按末词分状态，几条路径要在末词上不同才都留下来：ni + ta → 你他 / 你她 / 你它
    let mut router = router_with_scorer("你它");
    let (_, _, frame) = type_letters(&mut router, "nita");
    // 按键时只按词级模型：他 的词频高，首选是「你他」
    assert_eq!(candidate_texts(&frame).first(), Some(&"你他"));
    // 在等防抖，工人循环该在 80 ms 内醒来
    assert!(router.next_tick() <= std::time::Duration::from_millis(80));

    let frame = tick_until_first(&mut router, "你它", std::time::Duration::from_secs(3));
    assert_eq!(
        candidate_texts(&frame).first(),
        Some(&"你它"),
        "停顿后模型偏爱的整句应换到首位，实际：{:?}",
        candidate_texts(&frame)
    );
    // 换完不再等；空闲节拍回到看配置文件的一秒
    assert_eq!(router.next_tick(), std::time::Duration::from_secs(1));
}

#[test]
fn local_model_does_not_touch_a_navigated_page() {
    let mut router = router_with_scorer("你它");
    type_letters(&mut router, "nita");
    // 用户动过高亮：模型的结果只留在缓存里，不换正在看的这页
    let (_, _, frame) = press(&mut router, KeyEvent::new(0x28, None, Default::default())); // VK_DOWN
    assert_eq!(candidate_texts(&frame).first(), Some(&"你他"));
    // 防抖 80 ms + 假模型立即回分，300 ms 足够等到结果；首选仍是原来的
    let frame = tick_until_first(&mut router, "你它", std::time::Duration::from_millis(300));
    assert_eq!(candidate_texts(&frame).first(), Some(&"你他"));
}

#[test]
fn surrounding_text_arriving_after_the_first_key_still_rescoring() {
    let mut router = router_with_scorer("你它");
    type_letters(&mut router, "nita");
    // DLL 在起组句的编辑会话里读到前文、按键之后才送来：前文换了，缓存按旧前文记的作废，要能重新排期
    assert_eq!(
        router.handle(ClientMessage::Surrounding {
            session: SESSION,
            text: "今天".to_owned(),
        }),
        None
    );
    assert!(router.next_tick() <= std::time::Duration::from_millis(80));
    let frame = tick_until_first(&mut router, "你它", std::time::Duration::from_secs(3));
    assert_eq!(candidate_texts(&frame).first(), Some(&"你它"));
    // 别的会话送来的前文不影响聚焦会话
    assert_eq!(
        router.handle(ClientMessage::Surrounding {
            session: SessionId(9),
            text: "无关".to_owned(),
        }),
        None
    );
}

/// DLL 报来「私密输入框」：Engine 进私密（不学不记不发云端），焦点换到别的会话按那个会话的状态重设，切回来再进。
#[test]
fn privacy_follows_the_focused_session() {
    let mut router = router();
    // 真实顺序：第一键起组句，DLL 在那次编辑会话里判出私密再报来
    let (outcome, commit, _) = type_letters(&mut router, "kaifa");
    assert_eq!(outcome, KeyOutcome::Consumed);
    assert_eq!(commit, None);
    assert!(!router.is_private());
    assert_eq!(
        router.handle(ClientMessage::Privacy {
            session: SESSION,
            private: true,
        }),
        None
    );
    assert!(router.is_private());
    // 私密中照常上屏
    let (_, commit, _) = press(&mut router, digit(1));
    assert!(commit.is_some());
    // 另一个会话开进来拿焦点：它不私密
    open_session(&mut router, SessionId(2), None);
    press_in(&mut router, SessionId(2), letter('k'));
    assert!(!router.is_private());
    // 焦点回到第一个会话：仍是私密
    press_in(&mut router, SESSION, letter('k'));
    assert!(router.is_private());
    // 报不私密了
    assert_eq!(
        router.handle(ClientMessage::Privacy {
            session: SESSION,
            private: false,
        }),
        None
    );
    assert!(!router.is_private());
    // 别的会话的私密状态不影响聚焦会话
    assert_eq!(
        router.handle(ClientMessage::Privacy {
            session: SessionId(9),
            private: true,
        }),
        None
    );
    assert!(!router.is_private());
}

/// 实际 Server 按键→后台重排→Poll 路径：停键后不能再自己提交模型任务。
#[test]
fn stopped_mixed_input_keeps_the_model_idle_during_repeated_polls() {
    use qingjian_dictionary::{Dictionary, WordList};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    struct Counts(Arc<AtomicUsize>);
    impl qingjian_core::sentence::SentenceScorer for Counts {
        fn score(&self, _context: &str, _keys: &str, texts: &[&str]) -> Vec<f64> {
            self.0.fetch_add(1, Ordering::SeqCst);
            texts
                .iter()
                .map(|t| if *t == "我的" { -1.0 } else { -20.0 })
                .collect()
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let dict = Dictionary::parse("我\two\t900000\n的\tde\t800000\n我的\two de\t500000\n大\tda\t50000\n塔\tta\t3000\n巴\tba\t3000\n瑟\tse\t500\n").unwrap();
    let engine = qingjian_core::Engine::new(dict)
        .with_english(WordList::parse("database\tdatabase\t4310\n").unwrap())
        .with_async_sentence_scorer(Box::new(Counts(calls.clone())), None, None, None);
    let mut router = Router::new(engine, RouterConfig::default());
    open_session(&mut router, SESSION, None);
    type_letters(&mut router, "wodedatabase");
    let started = Instant::now();
    while calls.load(Ordering::SeqCst) == 0 || router.next_tick() != Duration::from_secs(1) {
        assert!(started.elapsed() < Duration::from_secs(3), "重排未收敛");
        std::thread::sleep(Duration::from_millis(20));
        router.tick();
        router.handle(ClientMessage::Poll { session: SESSION });
    }
    let settled = calls.load(Ordering::SeqCst);
    for _ in 0..200 {
        router.tick();
        let frame = match router.handle(ClientMessage::Poll { session: SESSION }) {
            Some(ServerMessage::Update { frame, .. }) => frame,
            other => panic!("expected Update, got {other:?}"),
        };
        assert_eq!(candidate_texts(&frame).first(), Some(&"我的database"));
        assert_eq!(router.next_tick(), Duration::from_secs(1));
    }
    assert_eq!(calls.load(Ordering::SeqCst), settled);
}
