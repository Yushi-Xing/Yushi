//! 搜索宿主内候选窗的布局：限制在父窗口客户区，优先放在输入行上方。

use windows::Win32::Foundation::RECT;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Layout {
    pub rect: RECT,

    pub row_height: i32,

    pub header_height: i32,

    pub padding: i32,
}

impl Layout {
    pub fn new(client: RECT, anchor: RECT, count: usize, dpi: u32) -> Option<Self> {
        if count == 0 || count > 9 {
            return None;
        }
        let scale = |value: i32| (value * dpi.clamp(96, 480) as i32 / 96).max(1);
        let padding = scale(8);
        let row_height = scale(32);
        let header_height = scale(32);
        let width = scale(480).min(client.right - client.left - 2 * padding);
        let height = header_height + row_height * count as i32 + padding;
        if width < scale(120) || height + 2 * padding > client.bottom - client.top {
            return None;
        }
        let left = anchor
            .left
            .clamp(client.left + padding, client.right - width - padding);
        let above = anchor.top.saturating_sub(height).saturating_sub(scale(4));
        let top = if above >= client.top + padding {
            above
        } else {
            anchor.bottom.saturating_add(scale(4))
        }
        .clamp(client.top + padding, client.bottom - height - padding);
        Some(Self {
            rect: RECT {
                left,
                top,
                right: left + width,
                bottom: top + height,
            },
            row_height,
            header_height,
            padding,
        })
    }

    pub fn hit(&self, x: i32, y: i32, count: usize) -> Option<usize> {
        let index = (y - self.header_height) / self.row_height;
        (x >= 0 && x < self.rect.right - self.rect.left && y >= self.header_height)
            .then_some(index as usize)
            .filter(|index| *index < count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_bar_below_panel_places_all_candidates_inside_panel() {
        let client = RECT {
            left: 0,
            top: 0,
            right: 800,
            bottom: 798,
        };
        let anchor = RECT {
            left: 80,
            top: 815,
            right: 250,
            bottom: 850,
        };
        for dpi in [96, 120, 144, 192] {
            let layout = Layout::new(client, anchor, 9, dpi).unwrap();
            assert!(layout.rect.left >= 0 && layout.rect.right <= client.right);
            assert!(layout.rect.top >= 0 && layout.rect.bottom <= client.bottom);
            assert!(layout.rect.bottom < anchor.top);
        }
    }

    #[test]
    fn edge_coordinates_and_small_hosts_cannot_panic_or_clip() {
        let client = RECT {
            left: 0,
            top: 0,
            right: 500,
            bottom: 400,
        };
        for (x, y) in [(-800, -400), (1000, 900), (80, 20)] {
            let anchor = RECT {
                left: x,
                top: y,
                right: x + 30,
                bottom: y + 25,
            };
            let layout = Layout::new(client, anchor, 3, 144).unwrap();
            assert!(layout.rect.left >= 0 && layout.rect.right <= client.right);
            assert!(layout.rect.top >= 0 && layout.rect.bottom <= client.bottom);
        }
        assert!(Layout::new(RECT::default(), RECT::default(), 9, 96).is_none());
        assert!(Layout::new(client, RECT::default(), 0, 96).is_none());
        assert!(Layout::new(client, RECT::default(), 10, 96).is_none());
    }

    #[test]
    fn header_padding_and_outside_clicks_do_not_select_candidates() {
        let client = RECT {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        };
        let layout = Layout::new(client, client, 3, 96).unwrap();
        assert_eq!(layout.hit(10, layout.header_height, 3), Some(0));
        assert_eq!(
            layout.hit(10, layout.header_height + 2 * layout.row_height, 3),
            Some(2)
        );
        assert_eq!(layout.hit(10, layout.header_height - 1, 3), None);
        assert_eq!(layout.hit(-1, layout.header_height, 3), None);
        assert_eq!(
            layout.hit(10, layout.header_height + 3 * layout.row_height, 3),
            None
        );
    }
}
