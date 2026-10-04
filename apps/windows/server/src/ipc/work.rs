//! 工人线程收到的连接消息、断开清理与状态条操作。

use std::sync::mpsc::Sender;

use qingjian_platform::protocol::{ClientMessage, ServerMessage, SessionId};

use crate::dispatch::StatusEvent;

/// 工人线程（独占 [`crate::dispatch::Router`]）的一件活：DLL 的一条消息，或状态条上的一次操作。
pub enum Work {
    /// 某条连接收到的消息 + 回结果的通道（`None` 表示不用回话）。
    Client(ClientMessage, Sender<Option<ServerMessage>>),

    /// 连接退出时清理它拥有的全部会话。
    Disconnect(Vec<SessionId>),

    /// UI 线程发来的状态条操作。
    Status(StatusEvent),
}
