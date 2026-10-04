//! 卸载模型时，当前单批完成后不得继续计算剩余读法。

use super::*;
use std::sync::mpsc;
use std::time::Duration;

struct Blocks {
    started: mpsc::Sender<()>,

    release: std::sync::Mutex<mpsc::Receiver<()>>,
}

impl SentenceScorer for Blocks {
    fn score(&self, _context: &str, _keys: &str, texts: &[&str]) -> Vec<f64> {
        self.started.send(()).unwrap();
        self.release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        vec![-1.0; texts.len()]
    }
}

#[test]
fn unloading_stops_remaining_batches_after_the_current_call() {
    let (started_tx, started) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let mut worker = RescoreWorker::spawn(Box::new(Blocks {
        started: started_tx,
        release: std::sync::Mutex::new(release_rx),
    }));
    worker.submit(
        String::new(),
        1,
        (0..3)
            .map(|i| ScoreBatch {
                keys: i.to_string(),
                texts: vec!["候选".to_owned()],
                scores: Vec::new(),
            })
            .collect(),
        None,
    );
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    let handle = worker.handle.take().unwrap();
    drop(worker);
    release.send(()).unwrap();
    handle.join().unwrap();
    assert!(matches!(
        started.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
}
