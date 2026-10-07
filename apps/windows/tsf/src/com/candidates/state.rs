//! 候选帧、显示协商与宿主回调队列；COM 回调只排队，不重入管道读写。

use qingjian_platform::protocol::{CandidateAction, Frame};
use std::cell::{Cell, RefCell};
use windows::Win32::UI::TextServices::ITfDocumentMgr;

#[derive(Default)]
pub(super) struct State {
    pub frame: RefCell<Frame>,

    pub document: RefCell<Option<ITfDocumentMgr>>,

    pub shown: Cell<bool>,

    /// 候选已由本 DLL 的宿主子窗口显示，Server 的独立窗应隐藏。
    pub hosted: Cell<bool>,

    pub pages: RefCell<Vec<u32>>,

    pub actions: RefCell<Vec<CandidateAction>>,
}

impl State {
    pub fn queue(&self, action: CandidateAction) {
        let action = match action {
            CandidateAction::Visibility { own_window } => CandidateAction::Visibility {
                own_window: own_window && !self.hosted.get(),
            },
            action => action,
        };
        let mut actions = self.actions.borrow_mut();
        match &action {
            CandidateAction::Visibility { .. } => {
                actions.retain(|a| !matches!(a, CandidateAction::Visibility { .. }))
            }
            CandidateAction::Select { .. } => {
                actions.retain(|a| !matches!(a, CandidateAction::Select { .. }))
            }
            _ => actions.retain(|a| matches!(a, CandidateAction::Visibility { .. })),
        }
        actions.push(action);
    }

    pub fn selection(&self, index: usize, finalize: bool) -> Option<CandidateAction> {
        let frame = self.frame.borrow();
        let text = frame.candidates.items.get(index)?.text.clone();
        let page = frame.page;
        Some(if finalize {
            CandidateAction::Finalize {
                page,
                index,
                text,
                typed_keys: frame.typed_keys.clone(),
            }
        } else {
            CandidateAction::Select {
                page,
                index,
                text,
                typed_keys: frame.typed_keys.clone(),
            }
        })
    }
}
