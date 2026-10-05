//! TSF 候选访问与选择接口；指针写入先检查长度，所有选择以当前帧为边界。

use super::element::Element_Impl;
use qingjian_platform::protocol::CandidateAction;
use windows::Win32::Foundation::{E_INVALIDARG, E_POINTER, E_UNEXPECTED, LPARAM, WPARAM};
use windows::Win32::UI::TextServices::{
    ITfCandidateListUIElement_Impl, ITfCandidateListUIElementBehavior_Impl, ITfDocumentMgr,
    ITfIntegratableCandidateListUIElement_Impl, ITfUIElement_Impl, STYLE_ACTIVE_SELECTION,
    TfIntegratableCandidateListSelectionStyle,
};
use windows::core::{BOOL, BSTR, Error, GUID, Result};

impl ITfUIElement_Impl for Element_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("青简候选"))
    }
    fn GetGUID(&self) -> Result<GUID> {
        Ok(GUID::from_u128(0xab30738a_9b37_4d63_91df_2f7b74c5e86e))
    }
    fn Show(&self, show: BOOL) -> Result<()> {
        self.state.shown.set(show.as_bool());
        self.state.queue(CandidateAction::Visibility {
            own_window: show.as_bool(),
        });
        Ok(())
    }
    fn IsShown(&self) -> Result<BOOL> {
        Ok(self.state.shown.get().into())
    }
}

impl ITfCandidateListUIElement_Impl for Element_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        Ok(0x3f)
    }
    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        self.state
            .document
            .borrow()
            .clone()
            .ok_or_else(|| Error::from(E_UNEXPECTED))
    }
    fn GetCount(&self) -> Result<u32> {
        Ok(self.state.frame.borrow().candidates.items.len() as u32)
    }
    fn GetSelection(&self) -> Result<u32> {
        Ok(self.state.frame.borrow().highlight as u32)
    }
    fn GetString(&self, index: u32) -> Result<BSTR> {
        self.state
            .frame
            .borrow()
            .candidates
            .items
            .get(index as usize)
            .map(|c| BSTR::from(c.text.as_str()))
            .ok_or_else(|| Error::from(E_INVALIDARG))
    }
    fn GetPageIndex(&self, indexes: *mut u32, capacity: u32, count: *mut u32) -> Result<()> {
        if count.is_null() || (capacity != 0 && indexes.is_null()) {
            return Err(Error::from(E_POINTER));
        }
        let pages = self.state.pages.borrow();
        unsafe {
            count.write(pages.len() as u32);
        }
        if capacity == 0 {
            return Ok(());
        }
        if capacity < pages.len() as u32 {
            return Err(Error::from(E_INVALIDARG));
        }
        if !pages.is_empty() {
            unsafe {
                std::ptr::copy_nonoverlapping(pages.as_ptr(), indexes, pages.len());
            }
        }
        Ok(())
    }
    fn SetPageIndex(&self, indexes: *const u32, count: u32) -> Result<()> {
        let total = self.GetCount()?;
        if indexes.is_null() {
            return Err(Error::from(E_POINTER));
        }
        if count == 0 || count > total {
            return Err(Error::from(E_INVALIDARG));
        }
        let pages = unsafe { std::slice::from_raw_parts(indexes, count as usize) };
        if pages[0] != 0
            || pages.iter().any(|i| *i >= total)
            || pages.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(Error::from(E_INVALIDARG));
        }
        *self.state.pages.borrow_mut() = pages.to_vec();
        Ok(())
    }
    fn GetCurrentPage(&self) -> Result<u32> {
        let selected = self.GetSelection()?;
        Ok(self
            .state
            .pages
            .borrow()
            .partition_point(|i| *i <= selected)
            .saturating_sub(1) as u32)
    }
}

impl ITfCandidateListUIElementBehavior_Impl for Element_Impl {
    fn SetSelection(&self, index: u32) -> Result<()> {
        let action = self
            .state
            .selection(index as usize, false)
            .ok_or_else(|| Error::from(E_INVALIDARG))?;
        self.state.frame.borrow_mut().highlight = index as usize;
        self.state.queue(action);
        Ok(())
    }
    fn Finalize(&self) -> Result<()> {
        let index = self.state.frame.borrow().highlight;
        let action = self
            .state
            .selection(index, true)
            .ok_or_else(|| Error::from(E_INVALIDARG))?;
        self.state.queue(action);
        Ok(())
    }
    fn Abort(&self) -> Result<()> {
        self.state.queue(CandidateAction::Abort {
            typed_keys: self.state.frame.borrow().typed_keys.clone(),
        });
        Ok(())
    }
}

impl ITfIntegratableCandidateListUIElement_Impl for Element_Impl {
    fn SetIntegrationStyle(&self, _style: &GUID) -> Result<()> {
        Ok(())
    }
    fn GetSelectionStyle(&self) -> Result<TfIntegratableCandidateListSelectionStyle> {
        Ok(STYLE_ACTIVE_SELECTION)
    }
    fn OnKeyDown(&self, _key: WPARAM, _flags: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }
    fn ShowCandidateNumbers(&self) -> Result<BOOL> {
        Ok(true.into())
    }
    fn FinalizeExactCompositionString(&self) -> Result<()> {
        self.state.queue(CandidateAction::Raw {
            typed_keys: self.state.frame.borrow().typed_keys.clone(),
        });
        Ok(())
    }
}
