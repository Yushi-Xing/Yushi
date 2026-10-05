//! 搜索候选快照及其枚举器跨出回调后仍持有 DLL 生命周期。

use std::rc::Rc;

pub(super) struct Lease;

impl Lease {
    pub fn new() -> Rc<Self> {
        crate::com::lock_module();
        Rc::new(Self)
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        crate::com::unlock_module();
    }
}
