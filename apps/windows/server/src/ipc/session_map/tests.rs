//! 会话映射的连接边界、额度与完整消息集合。

use super::{MAX_SESSIONS, SessionMap};
use qingjian_platform::protocol::{
    ClientMessage, IndicatorCommand, KeyEvent, ScreenRect, SessionId,
};

fn open(session: SessionId) -> ClientMessage {
    ClientMessage::OpenSession {
        session,
        app: None,
        protocol: 0,
    }
}

fn requests(session: SessionId) -> Vec<ClientMessage> {
    let rect = ScreenRect {
        left: 0,
        top: 0,
        right: 10,
        bottom: 10,
    };
    vec![
        ClientMessage::Key {
            session,
            event: KeyEvent::new(65, Some('a'), Default::default()),
        },
        ClientMessage::Poll { session },
        ClientMessage::Commit { session },
        ClientMessage::Surrounding {
            session,
            text: "前文".into(),
        },
        ClientMessage::Privacy {
            session,
            private: false,
        },
        ClientMessage::Selection {
            session,
            request: 1,
            text: "选区".into(),
            rect,
        },
        ClientMessage::PositionCandidates { session, rect },
        ClientMessage::HideCandidates { session },
        ClientMessage::ModeChanged {
            session,
            english: true,
        },
        ClientMessage::SyncMode { session },
        ClientMessage::Indicator {
            session,
            command: IndicatorCommand::OpenSettings,
        },
        ClientMessage::CloseSession { session },
    ]
}

#[test]
fn every_session_operation_requires_open_on_this_connection() {
    for external in [SessionId(0), SessionId(1), SessionId(u64::MAX)] {
        let mut map = SessionMap::default();
        for request in requests(external) {
            assert_eq!(
                map.map(request).unwrap_err().kind(),
                std::io::ErrorKind::PermissionDenied
            );
        }
        assert!(map.into_sessions().is_empty());
    }
}

#[test]
fn equal_external_ids_and_guessed_internal_ids_are_isolated() {
    let mut first = SessionMap::default();
    let mut second = SessionMap::default();
    let external = SessionId(u64::MAX);
    let (message, reply_id) = first.map(open(external)).unwrap();
    let first_id = message.session();
    assert_eq!(reply_id, external);
    let second_id = second.map(open(external)).unwrap().0.session();
    assert_ne!(first_id, second_id);
    for request in requests(external) {
        assert_eq!(second.map(request).unwrap().0.session(), second_id);
    }
    for request in requests(first_id) {
        assert!(second.map(request).is_err());
    }
    let guessed = second.map(open(first_id)).unwrap().0.session();
    assert_ne!(guessed, first_id);
}

#[test]
fn reopen_close_and_capacity_recovery() {
    let mut map = SessionMap::default();
    let id = map.map(open(SessionId(0))).unwrap().0.session();
    for _ in 0..1000 {
        assert_eq!(map.map(open(SessionId(0))).unwrap().0.session(), id);
    }
    for id in 1..MAX_SESSIONS {
        map.map(open(SessionId(id as u64))).unwrap();
    }
    assert!(map.map(open(SessionId(1000))).is_err());
    map.map(ClientMessage::CloseSession {
        session: SessionId(0),
    })
    .unwrap();
    map.closed(SessionId(0));
    assert!(
        map.map(ClientMessage::Poll {
            session: SessionId(0)
        })
        .is_err()
    );
    let new_id = map.map(open(SessionId(0))).unwrap().0.session();
    assert_ne!(id, new_id);
    assert_eq!(map.into_sessions().len(), MAX_SESSIONS);
}

#[test]
fn legacy_switch_notice_is_not_a_session_registration() {
    let mut map = SessionMap::default();
    let (notice, _) = map
        .map(ClientMessage::ImeSwitched {
            session: SessionId(55),
        })
        .unwrap();
    assert_eq!(notice.session(), SessionId(0));
    assert!(
        map.map(ClientMessage::Poll {
            session: SessionId(55)
        })
        .is_err()
    );
    assert!(map.into_sessions().is_empty());
}
