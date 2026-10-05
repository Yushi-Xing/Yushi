//! 模拟搜索宿主，验证显示协商、更新抑制与反注册生命周期。

use super::super::Candidates;
use super::frame;
use qingjian_platform::protocol::{CandidateAction, Frame};
use std::cell::RefCell;
use std::rc::Rc;
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::UI::TextServices::{
    IEnumTfUIElements, ITfUIElement, ITfUIElementMgr, ITfUIElementMgr_Impl,
};
use windows::core::{BOOL, Error, Ref, Result, implement};

#[implement(ITfUIElementMgr)]
struct Manager {
    calls: Rc<RefCell<Vec<&'static str>>>,

    fail: bool,
}

impl ITfUIElementMgr_Impl for Manager_Impl {
    fn BeginUIElement(
        &self,
        _element: Ref<ITfUIElement>,
        show: *mut BOOL,
        id: *mut u32,
    ) -> Result<()> {
        self.calls.borrow_mut().push("begin");
        if self.fail {
            return Err(Error::from(E_FAIL));
        }
        unsafe {
            show.write(false.into());
            id.write(42);
        }
        Ok(())
    }
    fn UpdateUIElement(&self, id: u32) -> Result<()> {
        assert_eq!(id, 42);
        self.calls.borrow_mut().push("update");
        Ok(())
    }
    fn EndUIElement(&self, id: u32) -> Result<()> {
        assert_eq!(id, 42);
        self.calls.borrow_mut().push("end");
        Ok(())
    }
    fn GetUIElement(&self, _id: u32) -> Result<ITfUIElement> {
        Err(Error::from(E_FAIL))
    }
    fn EnumUIElements(&self) -> Result<IEnumTfUIElements> {
        Err(Error::from(E_FAIL))
    }
}

#[test]
fn host_ownership_is_negotiated_once_and_unchanged_polls_do_not_redraw() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let manager: ITfUIElementMgr = Manager {
        calls: calls.clone(),
        fail: false,
    }
    .into();
    let candidates = Candidates::new();
    *candidates.manager.borrow_mut() = Some(manager);
    let mut input = frame();
    candidates.update(&input, None).unwrap();
    assert_eq!(
        candidates.take_actions(),
        [CandidateAction::Visibility { own_window: false }]
    );
    for _ in 0..50 {
        candidates.update(&input, None).unwrap();
    }
    input.highlight = 1;
    candidates.update(&input, None).unwrap();
    candidates.update(&Frame::default(), None).unwrap();
    candidates.deactivate();
    assert_eq!(*calls.borrow(), ["begin", "update", "update", "end"]);
}

#[test]
fn failing_host_returns_to_server_without_retrying_every_poll() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let manager: ITfUIElementMgr = Manager {
        calls: calls.clone(),
        fail: true,
    }
    .into();
    let candidates = Candidates::new();
    *candidates.manager.borrow_mut() = Some(manager);
    assert!(candidates.update(&frame(), None).is_err());
    for _ in 0..50 {
        candidates.update(&frame(), None).unwrap();
    }
    assert_eq!(*calls.borrow(), ["begin"]);
    assert_eq!(
        candidates.take_actions(),
        [CandidateAction::Visibility { own_window: true }]
    );
}
