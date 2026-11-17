//! 阻塞一个模型调用，验证换输入、取消与卸载后的剩余工作和结果。

use super::*;
use std::sync::mpsc;
use std::time::Duration;

struct Blocks {
    started: mpsc::Sender<String>,

    release: Mutex<mpsc::Receiver<()>>,
}

impl ChoiceModel for Blocks {
    fn align(&self, _: &str, _: &[String]) -> Option<Vec<u32>> {
        None
    }

    fn letter_units(&self, _: &str) -> Vec<u32> {
        Vec::new()
    }

    fn score(&self, context: &str, _: &[u32], texts: &[&str]) -> Vec<f32> {
        self.started.send(context.to_owned()).unwrap();
        self.release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        vec![0.0; texts.len()]
    }

    fn compose(&self, context: &str, _: &[(&str, bool)]) -> Option<super::super::CharTable> {
        self.score(context, &[], &[]);
        None
    }
}

fn requests(context: &str, count: usize) -> Job {
    Job::Compose(
        (0..count)
            .map(|index| ComposeRequest {
                key: format!("{context}-{index}"),
                context: context.to_owned(),
                syllables: vec![("ni".to_owned(), true)],
            })
            .collect(),
    )
}

fn worker() -> (ChoiceWorker, mpsc::Receiver<String>, mpsc::Sender<()>) {
    let (started_tx, started) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    (
        ChoiceWorker::spawn(
            Arc::new(Blocks {
                started: started_tx,
                release: Mutex::new(release_rx),
            }),
            WakeSlot::default(),
        ),
        started,
        release,
    )
}

#[test]
fn unloading_stops_remaining_compose_calls() {
    let (mut worker, started, release) = worker();
    worker.submit(requests("old", 3));
    assert_eq!(started.recv_timeout(Duration::from_secs(5)).unwrap(), "old");
    let handle = worker.handle.take().unwrap();
    drop(worker);
    release.send(()).unwrap();
    handle.join().unwrap();
    assert!(started.try_recv().is_err());
}

#[test]
fn rapid_requests_only_keep_the_latest_pending_job() {
    let (mut worker, started, release) = worker();
    worker.submit(requests("old", 3));
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    for index in 0..100 {
        worker.submit(requests(&format!("pending-{index}"), 2));
    }
    release.send(()).unwrap();
    assert_eq!(
        started.recv_timeout(Duration::from_secs(5)).unwrap(),
        "pending-99"
    );
    assert!(worker.poll().is_none(), "过期任务不能交回结果");
    let handle = worker.handle.take().unwrap();
    worker.cancel();
    release.send(()).unwrap();
    drop(worker);
    handle.join().unwrap();
    assert!(started.try_recv().is_err());
}

#[test]
fn cancel_discards_a_result_already_in_the_mailbox() {
    let (done_tx, done) = mpsc::channel();
    // 回调发生在结果入队之后，避免用睡眠猜线程时序。
    let wake = WakeSlot::default();
    wake.set(Some(Arc::new(move || {
        let _ = done_tx.send(());
    })));
    let (started_tx, started) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let worker = ChoiceWorker::spawn(
        Arc::new(Blocks {
            started: started_tx,
            release: Mutex::new(release_rx),
        }),
        wake,
    );
    worker.submit(requests("ready", 1));
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    release.send(()).unwrap();
    done.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.cancel();
    assert!(worker.poll().is_none());
}
