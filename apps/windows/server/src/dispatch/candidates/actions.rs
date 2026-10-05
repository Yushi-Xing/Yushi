//! 宿主候选列表操作：页内下标与文本须仍匹配，迟到点击不能选错词。

use crate::dispatch::Router;
use qingjian_platform::protocol::{
    CandidateAction, Frame, KeyEvent, KeyOutcome, ServerMessage, SessionId,
};

impl Router {
    pub(in crate::dispatch) fn handle_candidate_ui(
        &mut self,
        session: SessionId,
        action: CandidateAction,
    ) -> ServerMessage {
        let mut commit = None;
        let finalize = matches!(&action, CandidateAction::Finalize { .. });
        if let CandidateAction::Visibility { own_window } = action {
            if let Some(info) = self.sessions.get_mut(&session) {
                info.own_candidates = own_window;
            }
        } else if self.focused == Some(session) {
            match action {
                CandidateAction::Select {
                    page,
                    index,
                    text,
                    typed_keys,
                }
                | CandidateAction::Finalize {
                    page,
                    index,
                    text,
                    typed_keys,
                } => {
                    let frame = self.current_frame();
                    if typed_keys == frame.typed_keys
                        && page == frame.page
                        && frame
                            .candidates
                            .items
                            .get(index)
                            .is_some_and(|c| c.text == text)
                    {
                        self.highlight = page * self.config.page_size + index;
                        self.navigated = true;
                        if finalize {
                            commit = self.commit_index(self.highlight);
                            self.recompose();
                        }
                    }
                }
                CandidateAction::Abort { typed_keys }
                    if typed_keys == self.engine.composition().text() =>
                {
                    return self.handle_key(session, KeyEvent::new(0x1b, None, Default::default()));
                }
                CandidateAction::Raw { typed_keys }
                    if typed_keys == self.engine.composition().text() =>
                {
                    return self.handle_key(session, KeyEvent::new(0x0d, None, Default::default()));
                }
                CandidateAction::Abort { .. } | CandidateAction::Raw { .. } => {}
                CandidateAction::Visibility { .. } => unreachable!(),
            }
        }
        let frame = if self.focused == Some(session) {
            let shown = self.self_drawn_frame();
            self.reconcile_candidates(&shown);
            self.current_frame()
        } else {
            Frame::default()
        };
        ServerMessage::KeyResult {
            session,
            outcome: KeyOutcome::Consumed,
            commit,
            frame,
        }
    }
}
