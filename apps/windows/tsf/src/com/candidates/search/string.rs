//! 搜索候选字符串与列表内下标。

use super::lease::Lease;
use std::rc::Rc;
use windows::Win32::UI::TextServices::{ITfCandidateString, ITfCandidateString_Impl};
use windows::core::{BSTR, Result, implement};

#[implement(ITfCandidateString)]
pub(super) struct SearchString {
    pub text: String,

    pub index: u32,

    pub _lease: Rc<Lease>,
}

impl ITfCandidateString_Impl for SearchString_Impl {
    fn GetString(&self) -> Result<BSTR> {
        Ok(BSTR::from(self.text.as_str()))
    }
    fn GetIndex(&self) -> Result<u32> {
        Ok(self.index)
    }
}
