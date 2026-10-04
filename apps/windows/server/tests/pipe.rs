//! 真命名管道的端到端测试（仅 Windows）：监听线程 + 文件句柄客户端，走完「开会话 → 敲 nihao → 收候选」。

#![cfg(windows)]

use std::fs::OpenOptions;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use qingjian_core::Language;
use qingjian_platform::protocol::{
    ClientMessage, CodecError, KeyEvent, PROTOCOL_VERSION, ServerMessage, SessionId,
};
use qingjian_windows_server::ipc::pipe::serve_pipe;
use qingjian_windows_server::ipc::{read_message, write_message};
use qingjian_windows_server::{AssemblySpec, Router, RouterConfig, assembly};

const SESSION: SessionId = SessionId(1);

fn letter(c: char) -> KeyEvent {
    KeyEvent::new(c.to_ascii_uppercase() as u32, Some(c), Default::default())
}

/// 客户端重试打开管道，直到监听线程建好实例。
fn connect(name: &str) -> std::fs::File {
    for _ in 0..50 {
        match OpenOptions::new().read(true).write(true).open(name) {
            Ok(file) => return file,
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    }
    panic!("连不上管道 {name}");
}

#[test]
fn named_pipe_round_trips_the_open_type_loop() {
    let name = format!(r"\\.\pipe\qingjian-test-{}", std::process::id());

    // 监听线程服务完一个客户端后阻塞等下一个，随进程退出即可。
    let server_name = name.clone();
    thread::spawn(move || {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let engine = assembly::assemble(&AssemblySpec {
            glossary: Some((
                Language::English,
                root.join("assets/sample/glossary-en.tsv"),
            )),
            ..AssemblySpec::new(root.join("assets/sample/dict.tsv"))
        })
        .expect("assemble engine from sample data");
        let mut router = Router::new(engine, RouterConfig::default());
        let (work_tx, work_rx) = std::sync::mpsc::channel();
        let _ = serve_pipe(&server_name, &mut router, work_tx, work_rx);
    });

    let mut client = connect(&name);

    write_message(
        &mut client,
        &ClientMessage::OpenSession {
            session: SESSION,
            app: None,
            protocol: PROTOCOL_VERSION,
        },
    )
    .unwrap();
    for c in "nihao".chars() {
        write_message(
            &mut client,
            &ClientMessage::Key {
                session: SESSION,
                event: letter(c),
            },
        )
        .unwrap();
    }

    // 开会话先回一条 `SessionOpened`，之后五个按键各回一条 `KeyResult`。
    let mut last_frame = None;
    for _ in 0..6 {
        let message: ServerMessage = read_message(&mut client)
            .expect("read response")
            .expect("server closed early");
        if let ServerMessage::KeyResult { frame, .. } = message {
            last_frame = Some(frame);
        }
    }

    let frame = last_frame.expect("至少一条 KeyResult");
    let preedit: String = frame.preedit.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(preedit, "ni'hao");
    let texts: Vec<&str> = frame
        .candidates
        .items
        .iter()
        .map(|c| c.text.as_str())
        .collect();
    assert!(
        texts.contains(&"你好"),
        "候选里应有「你好」，实际：{texts:?}"
    );

    // 两条真实连接使用同一个线程编号：第二条不能读取或提交第一条的拼音。
    let mut other = connect(&name);
    write_message(
        &mut other,
        &ClientMessage::OpenSession {
            session: SESSION,
            app: None,
            protocol: PROTOCOL_VERSION,
        },
    )
    .unwrap();
    assert!(matches!(
        read_message::<_, ServerMessage>(&mut other).unwrap(),
        Some(ServerMessage::SessionOpened {
            session: SESSION,
            ..
        })
    ));
    write_message(&mut other, &ClientMessage::Poll { session: SESSION }).unwrap();
    let Some(ServerMessage::Update { frame, .. }) = read_message(&mut other).unwrap() else {
        panic!("expected Update");
    };
    assert!(frame.is_empty());
    write_message(&mut other, &ClientMessage::Commit { session: SESSION }).unwrap();
    assert!(matches!(
        read_message::<_, ServerMessage>(&mut other).unwrap(),
        Some(ServerMessage::Committed { text: None, .. })
    ));
    drop(other);
    write_message(&mut client, &ClientMessage::Poll { session: SESSION }).unwrap();
    let Some(ServerMessage::Update {
        frame,
        session: SESSION,
    }) = read_message(&mut client).unwrap()
    else {
        panic!("expected victim Update");
    };
    assert_eq!(
        frame
            .preedit
            .iter()
            .map(|part| part.text.as_str())
            .collect::<String>(),
        "ni'hao"
    );

    let mut unregistered = connect(&name);
    write_message(&mut unregistered, &ClientMessage::Poll { session: SESSION }).unwrap();
    // Windows 关闭命名管道可返回 ERROR_BROKEN_PIPE / ERROR_PIPE_NOT_CONNECTED；
    // 拒绝未登记请求时两者都表示连接已断开，不能假定字节流一定返回零字节。
    match read_message::<_, ServerMessage>(&mut unregistered) {
        Ok(None) => {}
        Err(CodecError::Io(error)) if matches!(error.raw_os_error(), Some(109 | 233)) => {}
        other => panic!("expected disconnected unauthorized pipe, got {other:?}"),
    }
}
