//! 经 ITfUIElementMgr 协商候选显示；宿主接管时更新原生列表，否则保留 Server 自绘。

mod element;
mod interfaces;
mod search;
mod state;
#[cfg(test)]
mod tests;

use self::element::Element;
use self::state::State;
use qingjian_platform::protocol::{CandidateAction, Frame};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows::Win32::UI::TextServices::{ITfContext, ITfThreadMgr, ITfUIElement, ITfUIElementMgr};
use windows::core::{ComObject, Interface, Result};

pub(crate) struct Candidates {
    manager: RefCell<Option<ITfUIElementMgr>>,

    id: Cell<Option<u32>>,

    state: Rc<State>,

    element: ComObject<Element>,
}

impl Candidates {
    pub fn new() -> Self {
        let state = Rc::new(State::default());
        Self {
            manager: RefCell::new(None),
            id: Cell::new(None),
            element: ComObject::new(Element::new(state.clone())),
            state,
        }
    }

    pub fn search_provider(&self) -> windows::core::IUnknown {
        let provider: windows::Win32::UI::TextServices::ITfFnSearchCandidateProvider =
            self.element.to_interface();
        provider.into()
    }

    pub fn activate(&self, manager: &ITfThreadMgr) {
        *self.manager.borrow_mut() = manager.cast().ok();
    }

    pub fn update(&self, frame: &Frame, context: Option<&ITfContext>) -> Result<()> {
        if frame.candidates.items.is_empty() {
            self.end();
            return Ok(());
        }
        if self.id.get().is_some() && *self.state.frame.borrow() == *frame {
            return Ok(());
        }
        *self.state.frame.borrow_mut() = frame.clone();
        *self.state.pages.borrow_mut() = vec![0];
        if let Some(context) = context {
            *self.state.document.borrow_mut() = unsafe { context.GetDocumentMgr() }.ok();
        }
        let manager = self.manager.borrow().clone();
        let Some(manager) = manager else {
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
                    self.state
                        .queue(CandidateAction::Visibility { own_window: true });
                    return Err(error);
                }
            }
            self.id.set(Some(id));
            self.state.shown.set(show.as_bool());
            self.state.queue(CandidateAction::Visibility {
                own_window: show.as_bool(),
            });
            id
        };
        unsafe {
            manager.UpdateUIElement(id)?;
        }
        Ok(())
    }

    pub fn take_actions(&self) -> Vec<CandidateAction> {
        std::mem::take(&mut *self.state.actions.borrow_mut())
    }

    pub fn end(&self) {
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
