//! 锁定产品数据和随包模型的停键性能探针；显式运行，不读用户配置、不学习、不联网。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use qingjian_core::sentence::SentenceScorer;
use qingjian_neural::{CharScorer, P2cScorer};
use qingjian_platform::protocol::{ClientMessage, KeyEvent, PROTOCOL_VERSION, SessionId};
use qingjian_windows_server::{AssemblySpec, LanguageModelFiles, Router, RouterConfig, assembly};

struct Counted {
    inner: P2cScorer,

    scores: Arc<AtomicUsize>,

    generations: Arc<AtomicUsize>,

    active: Arc<AtomicUsize>,

    failures: Arc<AtomicUsize>,
}

impl SentenceScorer for Counted {
    fn score(&self, context: &str, keys: &str, texts: &[&str]) -> Vec<f64> {
        self.scores.fetch_add(1, Ordering::SeqCst);
        self.active.fetch_add(1, Ordering::SeqCst);
        let scores = self.inner.score(context, keys, texts);
        self.active.fetch_sub(1, Ordering::SeqCst);
        if scores.len() != texts.len() {
            self.failures.fetch_add(1, Ordering::SeqCst);
        }
        scores
    }

    fn generate(&self, keys: &str, beam: usize, max_chars: usize) -> Vec<String> {
        self.generations.fetch_add(1, Ordering::SeqCst);
        self.active.fetch_add(1, Ordering::SeqCst);
        let texts = self.inner.generate(keys, beam, max_chars);
        self.active.fetch_sub(1, Ordering::SeqCst);
        texts
    }
}

#[cfg(windows)]
fn process_cpu_ms() -> Option<f64> {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    }
    .expect("current process CPU times");
    let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    Some((ticks(kernel) + ticks(user)) as f64 / 10000.0)
}

#[cfg(not(windows))]
fn process_cpu_ms() -> Option<f64> {
    None
}

#[test]
#[ignore = "requires verified product data and shipped P2C model"]
fn shipped_model_stays_idle_after_plain_and_mixed_input_settle() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let generated = root.join("data/generated");
    let mut spec = AssemblySpec::new(generated.join("dict.qj"));
    spec.english = Some(generated.join("english.tsv"));
    spec.language_model = Some(LanguageModelFiles::find(&generated).expect("product LM"));
    let scores = Arc::new(AtomicUsize::new(0));
    let generations = Arc::new(AtomicUsize::new(0));
    let active = Arc::new(AtomicUsize::new(0));
    let failures = Arc::new(AtomicUsize::new(0));
    let model =
        CharScorer::load(&root.join("data/models/hanzhang-tongbian/hanzhang-tongbian-small.qjm"))
            .expect("shipped model");
    let counted = Counted {
        inner: P2cScorer::new(model).expect("P2C model"),
        scores: scores.clone(),
        generations: generations.clone(),
        active: active.clone(),
        failures: failures.clone(),
    };
    let engine = assembly::assemble(&spec)
        .unwrap()
        .with_async_sentence_scorer(Box::new(counted), None, None, None);
    let mut router = Router::new(engine, RouterConfig::default());
    let session = SessionId(1);
    router.handle(ClientMessage::OpenSession {
        session,
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    for (case, keys) in [
        ("plain", "youcikanlaimeishenmewenti"),
        ("mixed", "wodedatabase"),
        ("mixed_typo", "nijuedezhegdiayzenmy"),
        ("last_initial", "huantaipingy"),
    ] {
        let baseline = scores.load(Ordering::SeqCst) + generations.load(Ordering::SeqCst);
        for c in keys.chars() {
            router.handle(ClientMessage::Key {
                session,
                event: KeyEvent::new(
                    u32::from(c.to_ascii_uppercase()),
                    Some(c),
                    Default::default(),
                ),
            });
        }
        let started = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(20));
            router.tick();
            router.handle(ClientMessage::Poll { session });
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "真实模型任务未收敛: {case}"
            );
            if started.elapsed() >= Duration::from_millis(100)
                && router.next_tick() == Duration::from_secs(1)
                && active.load(Ordering::SeqCst) == 0
            {
                break;
            }
        }
        let settled_ms = started.elapsed().as_millis();
        let settled = scores.load(Ordering::SeqCst) + generations.load(Ordering::SeqCst);
        assert_eq!(failures.load(Ordering::SeqCst), 0, "真实模型打分失败");
        // 词库已经唯一解释的简拼允许不调用模型；两项基准输入必须真实走过推理。
        if matches!(case, "plain" | "mixed") {
            assert!(settled > baseline, "未实际触发模型: {case}");
        }
        let cpu = process_cpu_ms();
        let idle = Instant::now();
        while idle.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(80));
            router.tick();
            router.handle(ClientMessage::Poll { session });
            assert_eq!(
                scores.load(Ordering::SeqCst) + generations.load(Ordering::SeqCst),
                settled,
                "停键后又调用模型: {case}"
            );
            assert_eq!(active.load(Ordering::SeqCst), 0);
        }
        let idle_cpu_ms = cpu
            .zip(process_cpu_ms())
            .map(|(before, after)| after - before);
        println!(
            "model_idle case={case} calls={} settled_ms={settled_ms} idle_new_calls=0 idle_cpu_ms={idle_cpu_ms:?}",
            settled - baseline
        );
        router.handle(ClientMessage::Commit { session });
    }
}
