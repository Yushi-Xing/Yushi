//! 宿主选词回调经消息泵送 Server，再通过异步编辑会话写文档。

use super::TextService_Impl;
use crate::com::composition::preedit_string;
use crate::com::edit::request_update;
use crate::com::log::log;

impl TextService_Impl {
    pub(super) fn apply_candidate_actions(&self) {
        let actions = self.shared.candidates.take_actions();
        if !self.shared.foreground() {
            return;
        }
        for action in actions {
            let response = {
                let Ok(mut guard) = self.engine.try_borrow_mut() else {
                    return;
                };
                let Some(client) = guard.as_mut() else {
                    return;
                };
                match client.candidate_ui(action) {
                    Ok(response) => response,
                    Err(error) => {
                        log(&format!("宿主选词失败: {error}"));
                        return;
                    }
                }
            };
            let context = self.shared.last_context();
            let _ = self
                .shared
                .candidates
                .update(&response.frame, context.as_ref());
            self.shared.set_composing(!response.frame.is_empty());
            if response.commit.is_some() || response.frame.is_empty() {
                let Some(context) = context else {
                    continue;
                };
                let preedit = if response.frame.preedit_mode.inline() {
                    preedit_string(&response.frame)
                } else {
                    String::new()
                };
                if let Err(error) = request_update(
                    &context,
                    self.client_id.get(),
                    self.engine.clone(),
                    self.shared.clone(),
                    response.commit,
                    preedit,
                ) {
                    log(&format!("宿主选词编辑会话失败: {error}"));
                }
            }
        }
    }
}

impl windows::Win32::UI::TextServices::ITfFunctionProvider_Impl for TextService_Impl {
    fn GetType(&self) -> windows::core::Result<windows::core::GUID> {
        Ok(crate::com::CLSID_QINGJIAN)
    }
    fn GetDescription(&self) -> windows::core::Result<windows::core::BSTR> {
        Ok(windows::core::BSTR::from("青简搜索集成"))
    }
    fn GetFunction(
        &self,
        guid: *const windows::core::GUID,
        iid: *const windows::core::GUID,
    ) -> windows::core::Result<windows::core::IUnknown> {
        use windows::Win32::Foundation::{E_NOINTERFACE, E_POINTER};
        use windows::Win32::UI::TextServices::ITfFnSearchCandidateProvider;
        use windows::core::{Error, GUID, Interface};
        if guid.is_null() || iid.is_null() {
            return Err(Error::from(E_POINTER));
        }
        if unsafe { *guid } != GUID::zeroed()
            || unsafe { *iid } != ITfFnSearchCandidateProvider::IID
        {
            return Err(Error::from(E_NOINTERFACE));
        }
        Ok(self.shared.candidates.search_provider())
    }
}
