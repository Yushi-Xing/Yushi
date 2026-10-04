//! 真帧传输的越权拒绝、会话清理与回包编号兼容性。

use std::io::{self, Cursor, Read, Write};
use std::path::PathBuf;

use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, PROTOCOL_VERSION, ServerMessage, SessionId,
};
use qingjian_windows_server::ipc::{read_message, serve, write_message};
use qingjian_windows_server::{AssemblySpec, Router, RouterConfig, assembly};

struct Stream {
    input: Cursor<Vec<u8>>,

    output: Vec<u8>,

    fail_write: bool,

    chunk: usize,
}

impl Read for Stream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = bytes.len().min(self.chunk);
        self.input.read(&mut bytes[..count])
    }
}

impl Write for Stream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "test disconnect"));
        }
        let count = bytes.len().min(self.chunk);
        self.output.write(&bytes[..count])
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn stream(messages: &[ClientMessage]) -> Stream {
    let mut input = Vec::new();
    for message in messages {
        write_message(&mut input, message).unwrap();
    }
    Stream {
        input: Cursor::new(input),
        output: Vec::new(),
        fail_write: false,
        chunk: usize::MAX,
    }
}

fn open(session: SessionId, protocol: u32) -> ClientMessage {
    ClientMessage::OpenSession {
        session,
        app: None,
        protocol,
    }
}

fn key(session: SessionId, c: char) -> ClientMessage {
    ClientMessage::Key {
        session,
        event: KeyEvent::new(c.to_ascii_uppercase() as u32, Some(c), Default::default()),
    }
}

fn router_with_victim() -> Router {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let engine =
        assembly::assemble(&AssemblySpec::new(root.join("assets/sample/dict.tsv"))).unwrap();
    let mut router = Router::new(engine, RouterConfig::default());
    router.handle(open(SessionId(u64::MAX), PROTOCOL_VERSION));
    for c in "nihao".chars() {
        router.handle(key(SessionId(u64::MAX), c));
    }
    router
}

fn assert_victim_intact(router: &mut Router) {
    assert_eq!(router.session_count(), 1);
    let Some(ServerMessage::Committed { text, .. }) = router.handle(ClientMessage::Commit {
        session: SessionId(u64::MAX),
    }) else {
        panic!("commit expected");
    };
    assert_eq!(text.as_deref(), Some("nihao"));
}

#[test]
fn unauthorized_wire_requests_never_read_or_mutate_existing_input() {
    for message in [
        ClientMessage::Poll {
            session: SessionId(u64::MAX),
        },
        ClientMessage::Commit {
            session: SessionId(u64::MAX),
        },
        ClientMessage::CloseSession {
            session: SessionId(u64::MAX),
        },
        ClientMessage::Privacy {
            session: SessionId(u64::MAX),
            private: true,
        },
        key(SessionId(u64::MAX), 'a'),
    ] {
        let mut router = router_with_victim();
        let mut attacker = stream(&[message]);
        assert!(serve(&mut attacker, &mut router).is_err());
        assert!(attacker.output.is_empty());
        assert!(!router.is_private());
        assert_victim_intact(&mut router);
    }
}

#[test]
fn duplicate_external_id_cannot_poll_commit_or_close_victim() {
    let mut router = router_with_victim();
    let mut attacker = stream(&[
        open(SessionId(u64::MAX), PROTOCOL_VERSION),
        ClientMessage::Poll {
            session: SessionId(u64::MAX),
        },
        ClientMessage::Commit {
            session: SessionId(u64::MAX),
        },
        ClientMessage::CloseSession {
            session: SessionId(u64::MAX),
        },
    ]);
    serve(&mut attacker, &mut router).unwrap();
    let mut replies = Cursor::new(attacker.output);
    assert!(matches!(
        read_message::<_, ServerMessage>(&mut replies).unwrap(),
        Some(ServerMessage::SessionOpened {
            session: SessionId(u64::MAX),
            ..
        })
    ));
    let Some(ServerMessage::Update { frame, .. }) = read_message(&mut replies).unwrap() else {
        panic!("update expected");
    };
    assert!(frame.is_empty());
    assert!(matches!(
        read_message::<_, ServerMessage>(&mut replies).unwrap(),
        Some(ServerMessage::Committed { text: None, .. })
    ));
    assert_victim_intact(&mut router);
}

#[test]
fn eof_bad_frame_truncation_and_write_failure_clean_only_own_sessions() {
    for tail in [
        vec![],
        vec![1, 0],
        vec![1, 0, 0, 0, b'!'],
        vec![5, 0, 0, 0, b'{'],
    ] {
        let mut router = router_with_victim();
        let mut client = stream(&[open(SessionId(0), PROTOCOL_VERSION)]);
        client.input.get_mut().extend_from_slice(&tail);
        let result = serve(&mut client, &mut router);
        assert_eq!(result.is_ok(), tail.is_empty());
        assert_victim_intact(&mut router);
    }
    let mut router = router_with_victim();
    let mut client = stream(&[open(SessionId(0), PROTOCOL_VERSION)]);
    client.fail_write = true;
    assert!(serve(&mut client, &mut router).is_err());
    assert_victim_intact(&mut router);
}

#[test]
fn current_and_legacy_open_keep_wire_response_order() {
    for protocol in [0, PROTOCOL_VERSION] {
        let mut router = router_with_victim();
        let mut client = stream(&[open(SessionId(0), protocol), key(SessionId(0), 'n')]);
        serve(&mut client, &mut router).unwrap();
        let mut replies = Cursor::new(client.output);
        if protocol != 0 {
            assert!(matches!(
                read_message::<_, ServerMessage>(&mut replies).unwrap(),
                Some(ServerMessage::SessionOpened {
                    session: SessionId(0),
                    ..
                })
            ));
        }
        assert!(matches!(
            read_message::<_, ServerMessage>(&mut replies).unwrap(),
            Some(ServerMessage::KeyResult {
                session: SessionId(0),
                ..
            })
        ));
        assert!(
            read_message::<_, ServerMessage>(&mut replies)
                .unwrap()
                .is_none()
        );
        assert_eq!(router.session_count(), 1);
        assert!(router.engine_mut().composition().is_empty());
    }
}

#[test]
fn fragmented_reads_and_writes_preserve_commit_and_connection_isolation() {
    for chunk in [1, 2, 3, 7, 64] {
        let mut router = router_with_victim();
        // 合法输入会切焦点，先结束旧文档组句；这里验证编号、分片和连接清理。
        assert_victim_intact(&mut router);
        let mut messages = vec![open(SessionId(7), PROTOCOL_VERSION)];
        messages.extend("nihao".chars().map(|c| key(SessionId(7), c)));
        messages.push(ClientMessage::Commit {
            session: SessionId(7),
        });
        let mut client = stream(&messages);
        client.chunk = chunk;
        serve(&mut client, &mut router).unwrap();
        let mut replies = Cursor::new(client.output);
        let mut count = 0;
        let mut committed = None;
        while let Some(message) = read_message::<_, ServerMessage>(&mut replies).unwrap() {
            assert_eq!(message.session(), SessionId(7));
            if let ServerMessage::Committed { text, .. } = message {
                committed = text;
            }
            count += 1;
        }
        assert_eq!(count, 7);
        assert_eq!(committed.as_deref(), Some("nihao"));
        assert_eq!(router.session_count(), 1);
        assert!(matches!(
            router.handle(ClientMessage::Commit {
                session: SessionId(u64::MAX)
            }),
            Some(ServerMessage::Committed { text: None, .. })
        ));
    }
}

#[test]
fn every_truncated_wire_offset_cleans_owned_sessions_without_touching_other_input() {
    let mut frame = Vec::new();
    write_message(&mut frame, &key(SessionId(7), 'n')).unwrap();
    for cut in 1..frame.len() {
        let mut router = router_with_victim();
        let mut client = stream(&[open(SessionId(7), PROTOCOL_VERSION)]);
        client.input.get_mut().extend_from_slice(&frame[..cut]);
        assert!(serve(&mut client, &mut router).is_err(), "cut={cut}");
        assert_victim_intact(&mut router);
    }
}
