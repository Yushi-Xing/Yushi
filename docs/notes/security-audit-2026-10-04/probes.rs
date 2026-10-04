//! 审计用原代码行为复现：只使用合成文本，不联网，不替代 Windows 控件与管道真机验证。

use std::sync::{Arc, Mutex};

use qingjian_core::{Engine, Prediction, PredictionPolicy, PredictionRequest, Predictor};
use qingjian_dictionary::Dictionary;
use qingjian_learning::InputLog;
use qingjian_platform::Config;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, ScreenRect, ServerMessage, SessionId,
};
use qingjian_windows_server::{Router, RouterConfig};

struct Capture(Arc<Mutex<Vec<PredictionRequest>>>);

impl Predictor for Capture {
    fn policy(&self) -> PredictionPolicy {
        PredictionPolicy::default()
    }

    fn submit(&mut self, request: PredictionRequest) {
        self.0.lock().unwrap().push(request);
    }

    fn poll(&mut self) -> Option<Prediction> {
        None
    }
}

fn router() -> Router {
    let dictionary = Dictionary::parse("你\tni\t100\n你好\tni hao\t100\n").unwrap();
    Router::new(Engine::new(dictionary), RouterConfig::default())
}

fn open(router: &mut Router, session: SessionId) {
    router.handle(ClientMessage::OpenSession {
        session,
        app: Some("SecurityAuditSynthetic.exe".into()),
        protocol: PROTOCOL_VERSION,
    });
}

#[test]
fn confirms_router_accepts_key_without_opening_a_session() {
    let mut router = router();
    assert_eq!(router.session_count(), 0);
    router.handle(ClientMessage::Key {
        session: SessionId(900001),
        event: KeyEvent::new(0x4e, Some('n'), KeyModifiers::default()),
    });
    assert_eq!(router.engine_mut().composition().text(), "n");
    assert_eq!(router.session_count(), 0);
}

#[test]
fn confirms_claimed_session_can_read_and_commit_composition() {
    let mut router = router();
    let session = SessionId(900002);
    open(&mut router, session);
    router.handle(ClientMessage::Key {
        session,
        event: KeyEvent::new(0x4e, Some('n'), KeyModifiers::default()),
    });
    // 实际 pipe.rs 将各连接消息交给同一入口，不传入可信连接身份。
    let poll = router.handle(ClientMessage::Poll { session });
    assert!(
        matches!(poll, Some(ServerMessage::Update { ref frame, .. }) if !frame.preedit.is_empty())
    );
    let commit = router.handle(ClientMessage::Commit { session });
    assert!(
        matches!(commit, Some(ServerMessage::Committed { text: Some(ref text), .. }) if text == "n")
    );
}

#[test]
fn confirms_translation_uses_unconfirmed_private_state() {
    let mut router = router();
    let requests = Arc::new(Mutex::new(Vec::new()));
    router
        .engine_mut()
        .set_predictor(Box::new(Capture(requests.clone())));
    let session = SessionId(900003);
    open(&mut router, session);
    assert!(!router.is_private());
    let config = RouterConfig::default();
    let combo = config.translate_selection;
    let reply = router.handle(ClientMessage::Key {
        session,
        event: KeyEvent::new(0x54, Some(combo.key), KeyModifiers::from(combo.modifiers)),
    });
    let request = match reply {
        Some(ServerMessage::RequestSelection { request, .. }) => request,
        other => panic!("expected selection request, got {other:?}"),
    };
    // DLL 的 SelectionSession 不先发送当前私密属性；这里只复现这条真实协议消息链。
    router.handle(ClientMessage::Selection {
        session,
        request,
        text: "SYNTHETIC_PRIVATE_SELECTION".into(),
        rect: ScreenRect {
            left: 0,
            top: 0,
            right: 1,
            bottom: 1,
        },
    });
    let captured = requests.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].text, "SYNTHETIC_PRIVATE_SELECTION");
}

#[test]
fn known_private_state_blocks_translation_control() {
    let mut router = router();
    let requests = Arc::new(Mutex::new(Vec::new()));
    router
        .engine_mut()
        .set_predictor(Box::new(Capture(requests.clone())));
    let session = SessionId(900004);
    open(&mut router, session);
    router.handle(ClientMessage::Privacy {
        session,
        private: true,
    });
    let combo = RouterConfig::default().translate_selection;
    let reply = router.handle(ClientMessage::Key {
        session,
        event: KeyEvent::new(0x54, Some(combo.key), KeyModifiers::from(combo.modifiers)),
    });
    if let Some(ServerMessage::RequestSelection { request, .. }) = reply {
        router.handle(ClientMessage::Selection {
            session,
            request,
            text: "SYNTHETIC_PRIVATE_SELECTION".into(),
            rect: ScreenRect {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            },
        });
    }
    assert!(requests.lock().unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn confirms_input_log_and_config_permissions_under_umask_022() {
    use std::os::unix::fs::PermissionsExt;
    let directory =
        std::env::temp_dir().join(format!("yushi-audit-permissions-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("input-log.jsonl");
    let log = InputLog::open(&path);
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o644, "run this probe with umask 022");
    let config_path = directory.join("config.toml");
    Config::set_value(&config_path, "predict", "api_key", "SYNTHETIC_NON_SECRET").unwrap();
    let config_mode = std::fs::metadata(&config_path)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(config_mode, 0o644);
    assert_eq!(
        Config::load(&config_path)
            .unwrap()
            .predict
            .api_key
            .as_deref(),
        Some("SYNTHETIC_NON_SECRET")
    );
    drop(log);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(config_path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
