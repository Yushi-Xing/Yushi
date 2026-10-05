//! 标准 TSF 候选 COM 对象，支持由搜索控件绘制列表和确认选择。

use super::state::State;
use std::rc::Rc;
use windows::Win32::UI::TextServices::{
    ITfCandidateListUIElement, ITfCandidateListUIElementBehavior, ITfFnSearchCandidateProvider,
    ITfIntegratableCandidateListUIElement, ITfUIElement,
};
use windows::core::implement;

#[implement(
    ITfUIElement,
    ITfCandidateListUIElement,
    ITfCandidateListUIElementBehavior,
    ITfIntegratableCandidateListUIElement,
    ITfFnSearchCandidateProvider
)]
pub(super) struct Element {
    pub state: Rc<State>,
}

impl Element {
    pub fn new(state: Rc<State>) -> Self {
        crate::com::lock_module();
        Self { state }
    }
}

impl Drop for Element {
    fn drop(&mut self) {
        crate::com::unlock_module();
    }
}
