//! 宿主接管候选列表时的窗口协商、选词和过期回调隔离。

use crate::support::{
    RecordingCandidates, SESSION, key_result, open_session, rect, router, type_letters,
};
use qingjian_platform::protocol::CandidateAction;

#[test]
fn protocol_seven_dlls_keep_auxiliary_segments_after_the_protocol_upgrade() {
    use qingjian_dictionary::AuxCodeTable;
    use qingjian_platform::protocol::{KeyEvent, PreeditKind};
    use std::sync::Arc;

    let mut router = router();
    let table = AuxCodeTable::from_pairs([("开发".to_owned(), "kf".to_owned())]).unwrap();
    router.engine_mut().set_aux_codes(vec![Arc::new(table)]);
    router.engine_mut().set_aux_enabled(true);
    router.handle(ClientMessage::OpenSession {
        session: SESSION,
        app: None,
        protocol: 7,
    });
    type_letters(&mut router, "kaifa");
    router.handle(ClientMessage::Key {
        session: SESSION,
        event: KeyEvent::new(0xba, Some(';'), Default::default()),
    });
    let (_, _, frame) = type_letters(&mut router, "k");
    assert!(frame.preedit.iter().any(|s| s.kind == PreeditKind::AuxCode));
}
use qingjian_platform::protocol::{ClientMessage, ServerMessage, SessionId};

#[test]
fn host_can_select_finalize_abort_and_preserve_raw_input() {
    let mut router = router();
    let (_, _, frame) = type_letters(&mut router, "kaifa");
    let text = frame.candidates.items[1].text.clone();
    let (_, commit, selected) = key_result(router.handle(ClientMessage::CandidateUi {
        session: SESSION,
        action: CandidateAction::Select {
            typed_keys: "kaifa".into(),
            page: frame.page,
            index: 1,
            text: text.clone(),
        },
    }));
    assert!(commit.is_none());
    assert_eq!(selected.highlight, 1);
    let (_, commit, _) = key_result(router.handle(ClientMessage::CandidateUi {
        session: SESSION,
        action: CandidateAction::Finalize {
            typed_keys: "kaifa".into(),
            page: frame.page,
            index: 1,
            text: text.clone(),
        },
    }));
    assert_eq!(commit, Some(text));
    type_letters(&mut router, "kaifa");
    let (_, commit, frame) = key_result(router.handle(ClientMessage::CandidateUi {
        session: SESSION,
        action: CandidateAction::Abort {
            typed_keys: "kaifa".into(),
        },
    }));
    assert!(commit.is_none());
    assert!(frame.is_empty());
    type_letters(&mut router, "kaifa");
    let (_, commit, frame) = key_result(router.handle(ClientMessage::CandidateUi {
        session: SESSION,
        action: CandidateAction::Raw {
            typed_keys: "kaifa".into(),
        },
    }));
    assert_eq!(commit.as_deref(), Some("kaifa"));
    assert!(frame.is_empty());
}

#[test]
fn stale_or_foreign_callbacks_never_commit_a_different_candidate() {
    let mut router = router();
    type_letters(&mut router, "kaifa");
    for (session, page, index, text) in [
        (SESSION, 0, 0, "旧候选"),
        (SESSION, 1, 0, "开发"),
        (SESSION, 0, usize::MAX, "开发"),
        (SessionId(99), 0, 0, "开发"),
    ] {
        let reply = router.handle(ClientMessage::CandidateUi {
            session,
            action: CandidateAction::Finalize {
                typed_keys: "kaifa".into(),
                page,
                index,
                text: text.into(),
            },
        });
        if let Some(ServerMessage::KeyResult { commit, .. }) = reply {
            assert!(commit.is_none());
        }
        assert_eq!(router.engine_mut().composition().text(), "kaifa");
    }
}

#[test]
fn native_host_owns_display_across_poll_position_and_other_session_changes() {
    let mut router = router();
    let recorded = RecordingCandidates::default();
    router.set_candidate_sink(Box::new(recorded.clone()));
    type_letters(&mut router, "kaifa");
    router.handle(ClientMessage::PositionCandidates {
        session: SESSION,
        rect: rect(),
    });
    let initial = recorded.0.lock().unwrap().len();
    assert!(initial > 0);
    router.handle(ClientMessage::CandidateUi {
        session: SESSION,
        action: CandidateAction::Visibility { own_window: false },
    });
    for _ in 0..50 {
        router.handle(ClientMessage::Poll { session: SESSION });
        router.handle(ClientMessage::PositionCandidates {
            session: SESSION,
            rect: rect(),
        });
    }
    assert_eq!(recorded.0.lock().unwrap().len(), initial);
    open_session(&mut router, SessionId(2), None);
    router.handle(ClientMessage::CandidateUi {
        session: SessionId(2),
        action: CandidateAction::Visibility { own_window: true },
    });
    assert_eq!(recorded.0.lock().unwrap().len(), initial);
    router.handle(ClientMessage::CandidateUi {
        session: SESSION,
        action: CandidateAction::Visibility { own_window: true },
    });
    assert!(recorded.0.lock().unwrap().len() > initial);
}

#[test]
fn callbacks_from_an_older_input_do_not_select_abort_or_commit_raw() {
    let mut router = router();
    let (_, _, frame) = type_letters(&mut router, "kaifa");
    for action in [
        CandidateAction::Finalize {
            typed_keys: "kf".into(),
            page: 0,
            index: 0,
            text: frame.candidates.items[0].text.clone(),
        },
        CandidateAction::Abort {
            typed_keys: "kf".into(),
        },
        CandidateAction::Raw {
            typed_keys: "kf".into(),
        },
    ] {
        let (_, commit, _) = key_result(router.handle(ClientMessage::CandidateUi {
            session: SESSION,
            action,
        }));
        assert!(commit.is_none());
        assert_eq!(router.engine_mut().composition().text(), "kaifa");
    }
}
