//! 原生子窗口回归：焦点、重复帧、绘制后点击、宿主销毁与资源释放。

use std::rc::Rc;

use qingjian_platform::protocol::{CandidateAction, ScreenRect};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetDC, GetUpdateRect, ReleaseDC, ValidateRect};
use windows::Win32::UI::HiDpi::GetThreadDpiAwarenessContext;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetForegroundWindow, GetParent, GetWindowRect, IsWindow,
    IsWindowVisible, SW_SHOWNOACTIVATE, SendMessageW, ShowWindow, WINDOW_EX_STYLE, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WS_POPUP,
};
use windows::core::w;

use super::{HostedWindow, VIEWS, local_window};
use crate::com::candidates::state::State;
use crate::com::candidates::tests::frame;

struct Host(HWND);

impl Host {
    fn new() -> Self {
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!(""),
                WS_POPUP,
                50,
                50,
                800,
                700,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        Self(hwnd)
    }

    fn anchor(&self) -> ScreenRect {
        let mut rect = RECT::default();
        unsafe {
            GetWindowRect(self.0, &mut rect).unwrap();
        }
        ScreenRect {
            left: rect.left + 80,
            top: rect.bottom - 60,
            right: rect.left + 160,
            bottom: rect.bottom - 40,
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

#[test]
fn child_belongs_to_host_and_mouse_activation_does_not_take_focus() {
    let host = Host::new();
    let before = unsafe { GetForegroundWindow() };
    let dpi_context = unsafe { GetThreadDpiAwarenessContext() };
    let state = Rc::new(State::default());
    let window = HostedWindow::new(host.0, state, &frame(), host.anchor()).unwrap();
    unsafe {
        assert_eq!(GetParent(window.hwnd).unwrap(), host.0);
        assert_eq!(
            SendMessageW(
                window.hwnd,
                WM_MOUSEACTIVATE,
                Some(WPARAM(0)),
                Some(LPARAM(0))
            )
            .0,
            3
        );
        assert_eq!(GetForegroundWindow(), before);
        assert_eq!(GetThreadDpiAwarenessContext(), dpi_context);
    }
    assert!(local_window(window.hwnd));
    assert!(window.valid_for(host.0));
    assert!(!window.valid_for(HWND::default()));
}

#[test]
fn clicks_use_the_displayed_frame_and_skip_pending_or_stale_content() {
    let host = Host::new();
    let state = Rc::new(State::default());
    let mut input = frame();
    let window = HostedWindow::new(host.0, state.clone(), &input, host.anchor()).unwrap();
    let layout = *window.view.layout.borrow();
    let x = 10;
    let y = layout.header_height + layout.row_height;
    let click = || unsafe {
        SendMessageW(
            window.hwnd,
            WM_LBUTTONUP,
            Some(WPARAM(0)),
            Some(LPARAM((x | y << 16) as isize)),
        );
    };
    click();
    assert!(state.actions.borrow().is_empty());
    let dc = unsafe { GetDC(Some(window.hwnd)) };
    window.view.paint(dc);
    unsafe {
        ReleaseDC(Some(window.hwnd), dc);
    }
    click();
    assert_eq!(
        state.actions.borrow().as_slice(),
        &[CandidateAction::Finalize {
            page: 0,
            index: 1,
            text: "太平洋".into(),
            typed_keys: "kaifa".into(),
        }]
    );
    state.actions.borrow_mut().clear();
    input.page = 1;
    input.typed_keys = "huantai".into();
    window.update(&input, host.anchor()).unwrap();
    click();
    assert!(state.actions.borrow().is_empty());
}

#[test]
fn identical_updates_do_not_repaint_and_parent_destruction_clears_callbacks() {
    let host = Host::new();
    unsafe {
        let _ = ShowWindow(host.0, SW_SHOWNOACTIVATE);
    }
    let state = Rc::new(State::default());
    let window = HostedWindow::new(host.0, state, &frame(), host.anchor()).unwrap();
    let hwnd = window.hwnd;
    let before = VIEWS.with(|views| views.borrow().len());
    unsafe {
        let _ = ValidateRect(Some(hwnd), None);
    }
    for _ in 0..50 {
        window.update(&frame(), host.anchor()).unwrap();
    }
    assert_eq!(VIEWS.with(|views| views.borrow().len()), before);
    assert!(!unsafe { GetUpdateRect(hwnd, None, false).as_bool() });
    assert!(unsafe { IsWindow(Some(hwnd)).as_bool() });
    window.hide();
    assert!(!unsafe { IsWindowVisible(hwnd).as_bool() });
    drop(host);
    assert!(!unsafe { IsWindow(Some(hwnd)).as_bool() });
    assert!(!window.valid_for(window.parent));
    assert!(!VIEWS.with(|views| views.borrow().contains_key(&(hwnd.0 as isize))));
    drop(window);
}

#[test]
fn rejected_parent_does_not_create_a_window_or_keep_a_callback() {
    let before = VIEWS.with(|views| views.borrow().len());
    assert!(
        HostedWindow::new(
            HWND::default(),
            Rc::new(State::default()),
            &frame(),
            ScreenRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0
            }
        )
        .is_err()
    );
    assert_eq!(VIEWS.with(|views| views.borrow().len()), before);
}
