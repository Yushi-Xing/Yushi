//! 上方候选只反转视觉行，点击目标仍然对应逻辑候选序号。

mod scenes;

use qingjian_render::{FontLibrary, HitRegion, HitTarget, Layout, Renderer, Theme};

fn cell(hits: &[HitRegion], index: usize) -> HitRegion {
    *hits
        .iter()
        .find(|r| r.target == HitTarget::Candidate(index))
        .unwrap()
}

#[test]
fn above_vertical_reverses_rows_without_changing_click_indices() {
    let library = FontLibrary::system("zh-CN").expect("system fonts");
    let mut renderer = Renderer::new(library);
    for theme in Theme::builtins() {
        let mut frame = scenes::nihao();
        frame.upward = true;
        renderer.forget();
        let rendered = renderer
            .render(&frame, Layout::Vertical, theme, 1.0)
            .unwrap();
        if let Some(folder) = std::env::var_os("QINGJIAN_UPWARD_PREVIEW") {
            let folder = std::path::PathBuf::from(folder);
            std::fs::create_dir_all(&folder).unwrap();
            rendered
                .pixmap
                .save_png(folder.join(format!("{}-above.png", theme.id())))
                .unwrap();
        }
        for i in 0..frame.rows.len() {
            let here = cell(&rendered.hits, i);
            if i > 0 {
                let below = cell(&rendered.hits, i - 1);
                assert!(here.y + here.height <= below.y + 0.5, "{} {i}", theme.id());
            }
            assert_eq!(
                rendered.hit_at(here.x + 2.0, here.y + here.height / 2.0),
                Some(HitTarget::Candidate(i)),
                "{}",
                theme.id()
            );
        }
        let preedit = rendered.preedit.as_ref().expect("preedit");
        assert!(
            preedit.y >= cell(&rendered.hits, 0).y + cell(&rendered.hits, 0).height - 0.5,
            "{}: preedit must be below the first candidate",
            theme.id()
        );
        for hit in rendered
            .hits
            .iter()
            .filter(|r| matches!(r.target, HitTarget::Page(_)))
        {
            assert!(
                hit.y < cell(&rendered.hits, 0).y,
                "{}: pager must not separate the first candidate from the input",
                theme.id()
            );
        }
    }
}

#[test]
fn above_horizontal_keeps_columns_and_reverses_only_matrix_rows() {
    let library = FontLibrary::system("zh-CN").expect("system fonts");
    let mut renderer = Renderer::new(library);
    for theme in Theme::builtins() {
        for columns in [0, 5] {
            let mut frame = scenes::matrix();
            frame.rows.truncate(13);
            frame.columns = columns;
            frame.upward = true;
            renderer.forget();
            let rendered = renderer
                .render(&frame, Layout::Horizontal, theme, 1.0)
                .unwrap();
            let preedit = rendered.preedit.as_ref().expect("preedit");
            assert!(
                preedit.y >= cell(&rendered.hits, 0).y + cell(&rendered.hits, 0).height - 0.5,
                "{}: preedit must be at the input edge",
                theme.id()
            );
            for i in 1..frame.rows.len() {
                let here = cell(&rendered.hits, i);
                if columns == 0 || i % columns != 0 {
                    let left = cell(&rendered.hits, i - 1);
                    assert!(
                        left.x < here.x && (left.y - here.y).abs() < 0.5,
                        "{} {i}",
                        theme.id()
                    );
                }
                if columns > 0 && i >= columns {
                    assert!(
                        here.y < cell(&rendered.hits, i - columns).y,
                        "{} {i}",
                        theme.id()
                    );
                }
            }
        }
    }
}

#[test]
fn below_vertical_keeps_the_preedit_above_the_candidates() {
    let library = FontLibrary::system("zh-CN").expect("system fonts");
    let mut renderer = Renderer::new(library);
    for theme in Theme::builtins() {
        renderer.forget();
        let frame = scenes::nihao();
        let rendered = renderer
            .render(&frame, Layout::Vertical, theme, 1.0)
            .unwrap();
        let preedit = rendered.preedit.as_ref().expect("preedit");
        assert!(
            preedit.y + preedit.height <= cell(&rendered.hits, 0).y + 0.5,
            "{}",
            theme.id()
        );
    }
}

#[test]
fn upward_and_reversed_together_keep_the_same_visual_and_click_order() {
    let library = FontLibrary::system("zh-CN").expect("system fonts");
    let mut renderer = Renderer::new(library);
    for theme in Theme::builtins() {
        let mut frame = scenes::nihao();
        frame.upward = true;
        renderer.forget();
        let upward = renderer
            .render(&frame, Layout::Vertical, theme, 1.0)
            .unwrap();
        frame.reversed = true;
        renderer.forget();
        let combined = renderer
            .render(&frame, Layout::Vertical, theme, 1.0)
            .unwrap();
        for i in 0..frame.rows.len() {
            let expected = cell(&upward.hits, i);
            let actual = cell(&combined.hits, i);
            assert_eq!(actual.y, expected.y, "{} candidate {i}", theme.id());
            assert_eq!(
                combined.hit_at(actual.x + 2.0, actual.y + actual.height / 2.0),
                Some(HitTarget::Candidate(i)),
            );
        }
        assert_eq!(combined.preedit.unwrap().y, upward.preedit.unwrap().y);
    }
}
