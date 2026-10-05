//! 搜索候选只提供当前输入的转换快照，不启动网络、学习或另一轮推理。

mod enumerator;
mod lease;
mod list;
mod string;

use self::list::SearchList;
use super::element::Element_Impl;
use windows::Win32::UI::TextServices::{
    ITfCandidateList, ITfFnSearchCandidateProvider_Impl, ITfFunction_Impl,
};
use windows::core::{BSTR, Result};

impl ITfFunction_Impl for Element_Impl {
    fn GetDisplayName(&self) -> Result<BSTR> {
        Ok(BSTR::from("青简搜索候选"))
    }
}

impl ITfFnSearchCandidateProvider_Impl for Element_Impl {
    fn GetSearchCandidates(&self, query: &BSTR, _application: &BSTR) -> Result<ITfCandidateList> {
        let frame = self.state.frame.borrow();
        let keys = frame.typed_keys.as_str();
        let text = query.to_string();
        let items = if !keys.is_empty()
            && (text == keys || frame.candidates.items.iter().any(|c| c.text == text))
        {
            qingjian_core::candidate::search_conversions(&frame.candidates.items, keys)
        } else {
            Vec::new()
        };
        Ok(SearchList::new(items).into())
    }
    fn SetResult(&self, _query: &BSTR, _application: &BSTR, _result: &BSTR) -> Result<()> {
        Ok(())
    }
}
