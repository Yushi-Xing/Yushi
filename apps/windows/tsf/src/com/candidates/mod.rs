//! 经 ITfUIElementMgr 协商候选显示；宿主接管时更新原生列表，否则保留 Server 自绘。

mod element;
mod hosted;
mod interfaces;
mod search;
mod state;
#[cfg(test)]
mod tests;

use self::element::Element;
use self::state::State;
use crate::com::log::log;
use qingjian_platform::protocol::{CandidateAction, Frame, ScreenRect};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows::Win32::UI::TextServices::{ITfContext, ITfThreadMgr, ITfUIElement, ITfUIElementMgr};
use windows::core::{ComObject, Interface, Result};

pub(crate) struct Candidates {
    manager: RefCell<Option<ITfUIElementMgr>>,

    id: Cell<Option<u32>>,

    state: Rc<State>,

    element: ComObject<Element>,

    host_fallback: bool,

    host_window: RefCell<Option<hosted::HostedWindow>>,

    anchor: Cell<Option<ScreenRect>>,

    host_failed: Cell<bool>,

    uiless: Cell<bool>,
}

impl Candidates {
    pub fn new() -> Self {
        let state = Rc::new(State::default());
        Self {
            manager: RefCell::new(None),
            id: Cell::new(None),
            element: ComObject::new(Element::new(state.clone())),
            state,
            host_fallback: hosted::search_host(),
            host_window: RefCell::new(None),
            anchor: Cell::new(None),
            host_failed: Cell::new(false),
            uiless: Cell::new(false),
        }
    }

    pub fn search_provider(&self) -> windows::core::IUnknown {
        let provider: windows::Win32::UI::TextServices::ITfFnSearchCandidateProvider =
            self.element.to_interface();
        provider.into()
    }

    pub fn activate(&self, manager: &ITfThreadMgr) {
        self.uiless.set(false);
        *self.manager.borrow_mut() = manager.cast().ok();
    }

    pub fn set_uiless(&self, enabled: bool) {
        self.uiless.set(enabled);
    }

    pub fn update(&self, frame: &Frame, context: Option<&ITfContext>) -> Result<()> {
        if frame.candidates.items.is_empty() {
            self.end();
            return Ok(());
        }
        if *self.state.frame.borrow() == *frame {
            self.sync_hosted(context);
            return Ok(());
        }
        *self.state.frame.borrow_mut() = frame.clone();
        *self.state.pages.borrow_mut() = vec![0];
        if let Some(context) = context {
            *self.state.document.borrow_mut() = unsafe { context.GetDocumentMgr() }.ok();
        }
        let manager = self.manager.borrow().clone();
        let Some(manager) = manager else {
            self.state.shown.set(!self.uiless.get());
            if self.uiless.get() {
                self.state
                    .queue(CandidateAction::Visibility { own_window: false });
            }
            self.sync_hosted(context);
            return Ok(());
        };
        let id = if let Some(id) = self.id.get() {
            id
        } else {
            let element: ITfUIElement = self.element.to_interface();
            let mut show = true.into();
            let mut id = 0;
            unsafe {
                if let Err(error) = manager.BeginUIElement(&element, &mut show, &mut id) {
                    self.manager.borrow_mut().take();
                    self.state.queue(CandidateAction::Visibility {
                        own_window: !self.uiless.get(),
                    });
                    self.state.shown.set(!self.uiless.get());
                    log("候选显示协商失败，使用自绘回退");
                    self.sync_hosted(context);
                    return Err(error);
                }
            }
            self.id.set(Some(id));
            self.state.shown.set(show.as_bool());
            self.state.queue(CandidateAction::Visibility {
                own_window: show.as_bool(),
            });
            log(&format!(
                "候选显示协商 own_ui={} search_host={}",
                show.as_bool(),
                self.host_fallback
            ));
            id
        };
        unsafe {
            manager.UpdateUIElement(id)?;
        }
        self.sync_hosted(context);
        Ok(())
    }

    pub fn position(&self, context: &ITfContext, anchor: ScreenRect) {
        self.anchor.set(Some(anchor));
        self.sync_hosted(Some(context));
    }

    /// 编辑会话先发送显隐，再报告位置，避免独立窗在搜索面板后闪现。
    pub fn take_visibility(&self) -> Option<CandidateAction> {
        let mut actions = self.state.actions.borrow_mut();
        let index = actions
            .iter()
            .rposition(|action| matches!(action, CandidateAction::Visibility { .. }))?;
        Some(actions.remove(index))
    }

    fn sync_hosted(&self, context: Option<&ITfContext>) {
        if !self.host_fallback {
            return;
        }
        if !self.state.shown.get() || self.state.frame.borrow().candidates.items.is_empty() {
            self.host_window.borrow_mut().take();
            self.state.hosted.set(false);
            return;
        }
        if self.host_failed.get() {
            return;
        }
        let Some(anchor) = self.anchor.get() else {
            return;
        };
        let Some(parent) = hosted::parent(context) else {
            if let Some(window) = self.host_window.borrow().as_ref() {
                window.hide();
            }
            return;
        };
        let frame = self.state.frame.borrow().clone();
        let mut window = self.host_window.borrow_mut();
        if window
            .as_ref()
            .is_some_and(|window| !window.valid_for(parent))
        {
            window.take();
        }
        let result = if let Some(window) = window.as_ref() {
            window.update(&frame, anchor)
        } else {
            hosted::HostedWindow::new(parent, self.state.clone(), &frame, anchor).map(|created| {
                *window = Some(created);
            })
        };
        match result {
            Ok(()) => {
                if !self.state.hosted.replace(true) {
                    log("搜索候选使用宿主内子窗口，不使用独立置顶窗");
                    self.state
                        .queue(CandidateAction::Visibility { own_window: false });
                }
            }
            Err(error) => {
                window.take();
                self.host_failed.set(true);
                self.state.hosted.set(false);
                self.state
                    .queue(CandidateAction::Visibility { own_window: true });
                log(&format!("搜索宿主内候选回退失败，本段不重试: {error}"));
            }
        }
    }

    pub fn take_actions(&self) -> Vec<CandidateAction> {
        self.sync_hosted(None);
        std::mem::take(&mut *self.state.actions.borrow_mut())
    }

    pub fn end(&self) {
        self.host_window.borrow_mut().take();
        self.state.hosted.set(false);
        self.anchor.set(None);
        self.host_failed.set(false);
        if let Some(id) = self.id.take()
            && let Some(manager) = self.manager.borrow().clone()
        {
            let _ = unsafe { manager.EndUIElement(id) };
        }
        self.state.shown.set(false);
        *self.state.frame.borrow_mut() = Frame::default();
        self.state.document.borrow_mut().take();
        self.state.actions.borrow_mut().clear();
    }

    pub fn deactivate(&self) {
        self.end();
        self.manager.borrow_mut().take();
    }
}
