//! 搜索宿主内的分层子窗口：与宿主共享窗口层级，不提权、不修改宿主样式。

mod layout;
#[cfg(test)]
mod tests;
mod view;

use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use qingjian_platform::protocol::{Frame, ScreenRect};
use windows::Win32::Foundation::{E_FAIL, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT, ScreenToClient,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, GetWindowDpiAwarenessContext, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::TextServices::ITfContext;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GA_ROOT, GetAncestor, GetClientRect,
    GetForegroundWindow, GetWindowThreadProcessId, HWND_TOP, IsWindow, IsWindowVisible, LWA_ALPHA,
    MA_NOACTIVATE, SW_HIDE, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_SHOWWINDOW,
    SetLayeredWindowAttributes, SetWindowPos, ShowWindow, WM_ERASEBKGND, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WM_NCDESTROY, WM_PAINT, WNDCLASSEXW, WS_CHILD, WS_CLIPSIBLINGS,
    WS_EX_LAYERED, WS_EX_NOACTIVATE,
};
use windows::core::{Error, PCWSTR, Result, w};

use self::layout::Layout;
use self::view::View;
use super::state::State;
use crate::com::window_class::WindowClass;

const CLASS_NAME: PCWSTR = w!("QingjianSearchCandidates");
static CLASS: WindowClass = WindowClass::new();

thread_local! {
    static VIEWS: RefCell<HashMap<isize, Rc<View>>> = RefCell::new(HashMap::new());
}

pub(super) struct HostedWindow {
    hwnd: HWND,

    pub parent: HWND,

    dpi: u32,

    view: Rc<View>,
}

impl HostedWindow {
    pub fn new(parent: HWND, state: Rc<State>, frame: &Frame, anchor: ScreenRect) -> Result<Self> {
        if !local_window(parent) {
            return Err(Error::from(E_FAIL));
        }
        CLASS.ensure(|| WNDCLASSEXW {
            lpfnWndProc: Some(wndproc),
            hInstance: crate::com::dll_instance(),
            lpszClassName: CLASS_NAME,
            ..Default::default()
        })?;
        let dpi = unsafe { GetDpiForWindow(parent) }.max(96);
        let layout = position(parent, anchor, frame.candidates.items.len(), dpi)?;
        // 不跨进程挂子窗口；DPI 上下文仅在创建期间与宿主保持一致。
        let previous =
            unsafe { SetThreadDpiAwarenessContext(GetWindowDpiAwarenessContext(parent)) };
        let created = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_LAYERED,
                CLASS_NAME,
                w!(""),
                WS_CHILD | WS_CLIPSIBLINGS,
                0,
                0,
                0,
                0,
                Some(parent),
                None,
                Some(crate::com::dll_instance()),
                None,
            )
        };
        if !previous.0.is_null() {
            unsafe {
                SetThreadDpiAwarenessContext(previous);
            }
        }
        let hwnd = created?;
        crate::com::lock_module();
        let view = Rc::new(View::new(state, layout, dpi));
        VIEWS.with(|views| {
            views.borrow_mut().insert(hwnd.0 as isize, view.clone());
        });
        let window = Self {
            hwnd,
            parent,
            dpi,
            view,
        };
        unsafe {
            SetLayeredWindowAttributes(hwnd, Default::default(), 255, LWA_ALPHA)?;
        }
        window.update(frame, anchor)?;
        Ok(window)
    }

    pub fn valid_for(&self, parent: HWND) -> bool {
        self.parent == parent
            && VIEWS.with(|views| views.borrow().contains_key(&(self.hwnd.0 as isize)))
            && unsafe { IsWindow(Some(self.hwnd)).as_bool() }
            && self.dpi == unsafe { GetDpiForWindow(parent) }.max(96)
    }

    pub fn update(&self, frame: &Frame, anchor: ScreenRect) -> Result<()> {
        let layout = position(self.parent, anchor, frame.candidates.items.len(), self.dpi)?;
        let changed = *self.view.frame.borrow() != *frame || *self.view.layout.borrow() != layout;
        *self.view.frame.borrow_mut() = frame.clone();
        *self.view.layout.borrow_mut() = layout;
        if changed || !unsafe { IsWindowVisible(self.hwnd).as_bool() } {
            let rect = layout.rect;
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOP),
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
                )?;
                let _ = InvalidateRect(Some(self.hwnd), None, false);
            }
        }
        Ok(())
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }
}

impl Drop for HostedWindow {
    fn drop(&mut self) {
        // 父窗口可能已销毁，NCDESTROY 已移除上下文；不要操作后来复用这个句柄的窗口。
        let alive = VIEWS.with(|views| views.borrow().contains_key(&(self.hwnd.0 as isize)));
        if alive {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
        VIEWS.with(|views| {
            views.borrow_mut().remove(&(self.hwnd.0 as isize));
        });
        crate::com::unlock_module();
    }
}

pub(super) fn search_host() -> bool {
    crate::com::host_app_name().is_some_and(|name| {
        name.eq_ignore_ascii_case("SearchHost.exe") || name.eq_ignore_ascii_case("SearchApp.exe")
    })
}

pub(super) fn parent(context: Option<&ITfContext>) -> Option<HWND> {
    let foreground = unsafe { GetForegroundWindow() };
    if !local_window(foreground) {
        return None;
    }
    let from_context =
        context.and_then(|context| unsafe { context.GetActiveView().ok()?.GetWnd().ok() });
    // 搜索输入框与结果面板可能是不同窗口；先用可见的前台面板，避免挂到隐藏或仅一行高的输入宿主。
    let root = unsafe { GetAncestor(foreground, GA_ROOT) };
    if local_window(root) && unsafe { IsWindowVisible(root).as_bool() } {
        return Some(root);
    }
    let root = unsafe { GetAncestor(from_context?, GA_ROOT) };
    (local_window(root) && unsafe { IsWindowVisible(root).as_bool() }).then_some(root)
}

fn local_window(window: HWND) -> bool {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    !window.is_invalid() && pid == unsafe { GetCurrentProcessId() }
}

fn position(parent: HWND, anchor: ScreenRect, count: usize, dpi: u32) -> Result<Layout> {
    let mut client = RECT::default();
    let mut begin = POINT {
        x: anchor.left,
        y: anchor.top,
    };
    let mut end = POINT {
        x: anchor.right,
        y: anchor.bottom,
    };
    unsafe {
        GetClientRect(parent, &mut client)?;
        if !ScreenToClient(parent, &mut begin).as_bool()
            || !ScreenToClient(parent, &mut end).as_bool()
        {
            return Err(Error::from(E_FAIL));
        }
    }
    Layout::new(
        client,
        RECT {
            left: begin.x,
            top: begin.y,
            right: end.x,
            bottom: end.y,
        },
        count,
        dpi,
    )
    .ok_or_else(|| Error::from(E_FAIL))
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let result = catch_unwind(AssertUnwindSafe(|| dispatch(hwnd, msg, lparam)));
    match result {
        Ok(Some(result)) => result,
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn dispatch(hwnd: HWND, msg: u32, lparam: LPARAM) -> Option<LRESULT> {
    if msg == WM_MOUSEACTIVATE {
        return Some(LRESULT(MA_NOACTIVATE as isize));
    }
    if msg == WM_ERASEBKGND {
        return Some(LRESULT(1));
    }
    if msg == WM_NCDESTROY {
        VIEWS.with(|views| {
            views.borrow_mut().remove(&(hwnd.0 as isize));
        });
        return None;
    }
    let view = VIEWS.with(|views| views.borrow().get(&(hwnd.0 as isize)).cloned())?;
    match msg {
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let dc = unsafe { BeginPaint(hwnd, &mut paint) };
            let painted = catch_unwind(AssertUnwindSafe(|| view.paint(dc)));
            unsafe {
                let _ = EndPaint(hwnd, &paint);
            }
            let _ = painted;
            Some(LRESULT(0))
        }
        WM_LBUTTONUP => {
            view.click(
                (lparam.0 as u16 as i16) as i32,
                ((lparam.0 >> 16) as u16 as i16) as i32,
            );
            Some(LRESULT(0))
        }
        _ => None,
    }
}
