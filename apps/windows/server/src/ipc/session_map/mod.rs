//! 每条传输连接独享会话映射；客户端编号只在该连接内有意义。

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};

use qingjian_platform::protocol::{ClientMessage, SessionId};

const MAX_SESSIONS: usize = 64;
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub(super) struct SessionMap {
    sessions: HashMap<SessionId, SessionId>,
}

impl SessionMap {
    pub(super) fn map(
        &mut self,
        mut message: ClientMessage,
    ) -> io::Result<(ClientMessage, SessionId)> {
        let external = message.session();
        // 旧 DLL 停用后用临时连接只通知状态条；此消息不读取或修改会话内容。
        if matches!(message, ClientMessage::ImeSwitched { .. }) {
            *message.session_mut() = SessionId(0);
            return Ok((message, external));
        }
        let internal = if matches!(message, ClientMessage::OpenSession { .. }) {
            if let Some(&internal) = self.sessions.get(&external) {
                internal
            } else {
                if self.sessions.len() >= MAX_SESSIONS {
                    return Err(io::Error::other("connection session limit exceeded"));
                }
                let id = NEXT_SESSION
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                    .map_err(|_| io::Error::other("session identifiers exhausted"))?;
                let internal = SessionId(id);
                self.sessions.insert(external, internal);
                internal
            }
        } else {
            *self.sessions.get(&external).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "session not owned by connection",
                )
            })?
        };
        *message.session_mut() = internal;
        // 关闭操作确认派发后再移除映射；派发失败时仍能在断开路径清理。
        Ok((message, external))
    }

    pub(super) fn closed(&mut self, external: SessionId) {
        self.sessions.remove(&external);
    }

    pub(super) fn into_sessions(self) -> Vec<SessionId> {
        self.sessions.into_values().collect()
    }
}

#[cfg(test)]
mod tests;
