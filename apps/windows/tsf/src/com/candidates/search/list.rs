//! 不可变的搜索候选列表，确认反馈不写学习文件。

use super::{enumerator::Enumerator, lease::Lease, string::SearchString};
use std::rc::Rc;
use windows::Win32::Foundation::E_INVALIDARG;
use windows::Win32::UI::TextServices::{
    IEnumTfCandidates, ITfCandidateList, ITfCandidateList_Impl, ITfCandidateString,
    TfCandidateResult,
};
use windows::core::{Error, Result, implement};

#[implement(ITfCandidateList)]
pub(super) struct SearchList {
    texts: Rc<Vec<String>>,

    lease: Rc<Lease>,
}

impl SearchList {
    pub fn new(texts: Vec<String>) -> Self {
        Self {
            texts: Rc::new(texts),
            lease: Lease::new(),
        }
    }
}

impl ITfCandidateList_Impl for SearchList_Impl {
    fn EnumCandidates(&self) -> Result<IEnumTfCandidates> {
        Ok(Enumerator::new(self.texts.clone(), self.lease.clone(), 0).into())
    }
    fn GetCandidate(&self, index: u32) -> Result<ITfCandidateString> {
        let text = self
            .texts
            .get(index as usize)
            .ok_or_else(|| Error::from(E_INVALIDARG))?;
        Ok(SearchString {
            text: text.clone(),
            index,
            _lease: self.lease.clone(),
        }
        .into())
    }
    fn GetCandidateNum(&self) -> Result<u32> {
        Ok(self.texts.len() as u32)
    }
    fn SetResult(&self, index: u32, _result: TfCandidateResult) -> Result<()> {
        if index as usize >= self.texts.len() {
            return Err(Error::from(E_INVALIDARG));
        }
        Ok(())
    }
}
