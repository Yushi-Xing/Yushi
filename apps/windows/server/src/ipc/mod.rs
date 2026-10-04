//! Server 与 DLL 之间的传输。线上帧格式在 [`qingjian_platform::protocol`]（两端共用），
//! 这层只提供双工字节流上的消息循环（[`serve`]）与具体传输（命名管道 [`pipe`]）。

#[cfg(windows)]
pub mod pipe;
mod session_map;
mod work;

pub use qingjian_platform::protocol::{CodecError, read_message, write_message};
pub use work::Work;

use std::io::{Read, Write};

use qingjian_platform::protocol::ClientMessage;

use crate::dispatch::Router;
use session_map::SessionMap;

/// 在一条已连上的双工流上服务一个客户端：读消息、交给 Router、写回，直到对端在帧边界关闭。
pub fn serve<S: Read + Write>(stream: &mut S, router: &mut Router) -> Result<(), CodecError> {
    let mut sessions = SessionMap::default();
    let result = (|| {
        while let Some(message) = read_message::<_, ClientMessage>(&mut *stream)? {
            let closing = matches!(message, ClientMessage::CloseSession { .. });
            let (message, external) = sessions.map(message)?;
            let response = router.handle(message);
            if closing {
                sessions.closed(external);
            }
            if let Some(mut response) = response {
                *response.session_mut() = external;
                write_message(&mut *stream, &response)?;
            }
        }
        Ok(())
    })();
    for session in sessions.into_sessions() {
        router.handle(ClientMessage::CloseSession { session });
    }
    result
}
