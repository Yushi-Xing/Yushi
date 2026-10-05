//! 搜索列表枚举：克隆保留游标，短读返回 S_FALSE；不跨快照取数据。

use super::{lease::Lease, string::SearchString};
use std::cell::Cell;
use std::rc::Rc;
use windows::Win32::Foundation::{E_POINTER, S_FALSE};
use windows::Win32::UI::TextServices::{
    IEnumTfCandidates, IEnumTfCandidates_Impl, ITfCandidateString,
};
use windows::core::{Error, Result, implement};

#[implement(IEnumTfCandidates)]
pub(super) struct Enumerator {
    texts: Rc<Vec<String>>,

    lease: Rc<Lease>,

    index: Cell<usize>,
}

impl Enumerator {
    pub fn new(texts: Rc<Vec<String>>, lease: Rc<Lease>, index: usize) -> Self {
        Self {
            texts,
            lease,
            index: Cell::new(index),
        }
    }
}

impl IEnumTfCandidates_Impl for Enumerator_Impl {
    fn Clone(&self) -> Result<IEnumTfCandidates> {
        Ok(Enumerator::new(self.texts.clone(), self.lease.clone(), self.index.get()).into())
    }
    fn Next(
        &self,
        count: u32,
        output: *mut Option<ITfCandidateString>,
        fetched: *mut u32,
    ) -> Result<()> {
        if (count != 0 && output.is_null()) || (count != 1 && fetched.is_null()) {
            return Err(Error::from(E_POINTER));
        }
        let available = self
            .texts
            .len()
            .saturating_sub(self.index.get())
            .min(count as usize);
        if !fetched.is_null() {
            unsafe {
                fetched.write(available as u32);
            }
        }
        for offset in 0..available {
            let index = self.index.get() + offset;
            let item = SearchString {
                text: self.texts[index].clone(),
                index: index as u32,
                _lease: self.lease.clone(),
            };
            unsafe {
                output.add(offset).write(Some(item.into()));
            }
        }
        self.index.set(self.index.get() + available);
        if available < count as usize {
            return Err(Error::from_hresult(S_FALSE));
        }
        Ok(())
    }
    fn Reset(&self) -> Result<()> {
        self.index.set(0);
        Ok(())
    }
    fn Skip(&self, count: u32) -> Result<()> {
        let next = self.index.get().saturating_add(count as usize);
        self.index.set(next.min(self.texts.len()));
        if next > self.texts.len() {
            return Err(Error::from_hresult(S_FALSE));
        }
        Ok(())
    }
}
