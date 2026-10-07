//! 搜索候选回退的 GDI 显示：只画 Server 已排序的帧，鼠标回调只排队。

use std::cell::RefCell;
use std::rc::Rc;

use qingjian_platform::ThemeMode;
use qingjian_platform::protocol::{CandidateAction, Frame};
use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET,
    DT_END_ELLIPSIS, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, FF_DONTCARE,
    FillRect, HDC, HFONT, OUT_TT_PRECIS, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
    VARIABLE_PITCH,
};
use windows::core::w;

use super::layout::Layout;
use crate::com::candidates::state::State;

pub(super) struct View {
    pub frame: RefCell<Frame>,

    pub layout: RefCell<Layout>,

    painted: RefCell<Option<(Frame, Layout)>>,

    state: Rc<State>,

    font: HFONT,
}

impl View {
    pub fn new(state: Rc<State>, layout: Layout, dpi: u32) -> Self {
        let font = unsafe {
            CreateFontW(
                -(16 * dpi.clamp(96, 480) as i32 / 96),
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_TT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                (VARIABLE_PITCH.0 | FF_DONTCARE.0) as u32,
                w!("Microsoft YaHei UI"),
            )
        };
        Self {
            frame: RefCell::new(Frame::default()),
            layout: RefCell::new(layout),
            painted: RefCell::new(None),
            state,
            font,
        }
    }

    pub fn paint(&self, dc: HDC) {
        let frame = self.frame.borrow();
        let layout = *self.layout.borrow();
        let dark = match frame.theme {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => windows_registry::CURRENT_USER
                .open(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
                .and_then(|key| key.get_u32("AppsUseLightTheme"))
                .is_ok_and(|value| value == 0),
        };
        let (background, foreground, highlight) = if dark {
            (rgb(36, 36, 40), rgb(245, 245, 245), rgb(56, 76, 108))
        } else {
            (rgb(248, 248, 250), rgb(30, 30, 35), rgb(220, 234, 252))
        };
        let width = layout.rect.right - layout.rect.left;
        fill(
            dc,
            RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: layout.rect.bottom - layout.rect.top,
            },
            background,
        );
        let old_font = unsafe { SelectObject(dc, self.font.into()) };
        unsafe {
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, foreground);
        }
        let header = frame
            .preedit
            .iter()
            .filter(|segment| segment.kind != qingjian_platform::protocol::PreeditKind::Corrected)
            .map(|segment| segment.text.as_str())
            .collect::<String>();
        let header = format!("{}  {}/{}", header, frame.page + 1, frame.page_count.max(1));
        text(
            dc,
            &header,
            RECT {
                left: layout.padding,
                top: 0,
                right: width - layout.padding,
                bottom: layout.header_height,
            },
        );
        for (index, candidate) in frame.candidates.items.iter().enumerate() {
            let top = layout.header_height + index as i32 * layout.row_height;
            let row = RECT {
                left: 0,
                top,
                right: width,
                bottom: top + layout.row_height,
            };
            if index == frame.highlight {
                fill(dc, row, highlight);
            }
            let mut label = format!("{}. {}", index + 1, candidate.text);
            if let Some(translation) = &candidate.translation {
                label.push_str("   ");
                label.push_str(
                    &translation
                        .senses()
                        .iter()
                        .map(|sense| sense.text.as_str())
                        .collect::<Vec<_>>()
                        .join("; "),
                );
            }
            if frame.aux_code_show
                && let Some(code) = &candidate.aux_code
            {
                label.push_str("   ");
                label.push_str(code);
            }
            text(
                dc,
                &label,
                RECT {
                    left: layout.padding,
                    right: width - layout.padding,
                    ..row
                },
            );
        }
        unsafe {
            SelectObject(dc, old_font);
        }
        *self.painted.borrow_mut() = Some((frame.clone(), layout));
    }

    pub fn click(&self, x: i32, y: i32) {
        let painted = self.painted.borrow();
        let Some((frame, layout)) = painted.as_ref() else {
            return;
        };
        // 已显示的帧发生变化时，旧画面点击不应误选新一页。
        if *frame != *self.frame.borrow() || *layout != *self.layout.borrow() {
            return;
        }
        let Some(index) = layout.hit(x, y, frame.candidates.items.len()) else {
            return;
        };
        self.state.queue(CandidateAction::Finalize {
            page: frame.page,
            index,
            text: frame.candidates.items[index].text.clone(),
            typed_keys: frame.typed_keys.clone(),
        });
    }
}

impl Drop for View {
    fn drop(&mut self) {
        if !self.font.is_invalid() {
            let _ = unsafe { DeleteObject(self.font.into()) };
        }
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | (g as u32) << 8 | (b as u32) << 16)
}

fn fill(dc: HDC, rect: RECT, color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(dc, &rect, brush);
        let _ = DeleteObject(brush.into());
    }
}

fn text(dc: HDC, value: &str, mut rect: RECT) {
    let mut value = value.encode_utf16().collect::<Vec<_>>();
    unsafe {
        DrawTextW(
            dc,
            &mut value,
            &mut rect,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }
}
