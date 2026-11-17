//! 用真正发给候选窗的帧回传布局方向，覆盖 UI 与按键路由之间的完整状态链。

use qingjian_platform::LayoutMode;
use qingjian_platform::protocol::Frame;

use crate::support::{
    CandidateMouse, ClientMessage, RecordingCandidates, Router, RouterConfig, SESSION,
    function_key, letter, press, rect, router_with, type_letters,
};

const UP: u32 = 0x26;
const DOWN: u32 = 0x28;

fn last_frame(sink: &RecordingCandidates) -> Frame {
    sink.0
        .lock()
        .unwrap()
        .last()
        .cloned()
        .expect("candidate window frame")
}

fn place(router: &mut Router, sink: &RecordingCandidates, upward: bool) {
    let shown = last_frame(sink);
    router.handle_candidate_mouse(CandidateMouse::Placement {
        upward,
        typed_keys: shown.typed_keys,
    });
}

#[test]
fn directions_follow_the_actual_vertical_window_frame_across_pages_and_side_changes() {
    let mut router = router_with(RouterConfig {
        layout: LayoutMode::Vertical,
        page_size: 2,
        ..RouterConfig::default()
    });
    let sink = RecordingCandidates::default();
    router.set_candidate_sink(Box::new(sink.clone()));
    let (_, _, initial) = type_letters(&mut router, "ni");
    assert!(
        initial.page_count > 1,
        "cross-page navigation needs multiple pages"
    );
    router.handle(ClientMessage::PositionCandidates {
        session: SESSION,
        rect: rect(),
    });
    assert_eq!(last_frame(&sink).typed_keys, "ni");
    place(&mut router, &sink, true);
    assert_eq!(press(&mut router, function_key(UP)).2.highlight, 1);
    let (_, _, next_page) = press(&mut router, function_key(UP));
    assert_eq!((next_page.page, next_page.highlight), (1, 0));
    let (_, _, previous_page) = press(&mut router, function_key(DOWN));
    assert_eq!((previous_page.page, previous_page.highlight), (0, 1));
    assert_eq!(press(&mut router, function_key(DOWN)).2.highlight, 0);
    // 同一段拼音的窗口换到下方，按 ↓ 应直接移到下一项。
    place(&mut router, &sink, false);
    assert_eq!(press(&mut router, function_key(DOWN)).2.highlight, 1);
    assert_eq!(press(&mut router, function_key(UP)).2.highlight, 0);
}

#[test]
fn delayed_placement_from_the_previous_input_cannot_override_the_new_direction() {
    let mut router = router_with(RouterConfig {
        layout: LayoutMode::Vertical,
        ..RouterConfig::default()
    });
    let sink = RecordingCandidates::default();
    router.set_candidate_sink(Box::new(sink.clone()));
    type_letters(&mut router, "nihao");
    router.handle(ClientMessage::PositionCandidates {
        session: SESSION,
        rect: rect(),
    });
    let old = last_frame(&sink);
    place(&mut router, &sink, true);
    press(&mut router, letter('m'));
    assert_eq!(last_frame(&sink).typed_keys, "nihaom");
    place(&mut router, &sink, false);
    router.handle_candidate_mouse(CandidateMouse::Placement {
        upward: true,
        typed_keys: old.typed_keys,
    });
    assert_eq!(press(&mut router, function_key(DOWN)).2.highlight, 1);
}

#[test]
fn disabled_vertical_reversal_keeps_downward_navigation_even_above_the_text() {
    let mut router = router_with(RouterConfig {
        layout: LayoutMode::Vertical,
        vertical_reverse_above: false,
        page_size: 2,
        ..RouterConfig::default()
    });
    let sink = RecordingCandidates::default();
    router.set_candidate_sink(Box::new(sink.clone()));
    type_letters(&mut router, "ni");
    router.handle(ClientMessage::PositionCandidates {
        session: SESSION,
        rect: rect(),
    });
    place(&mut router, &sink, true);
    assert_eq!(press(&mut router, function_key(DOWN)).2.highlight, 1);
    let (_, _, next) = press(&mut router, function_key(DOWN));
    assert_eq!((next.page, next.highlight), (1, 0));
    let (_, _, previous) = press(&mut router, function_key(UP));
    assert_eq!((previous.page, previous.highlight), (0, 1));
    assert_eq!(press(&mut router, function_key(UP)).2.highlight, 0);
    place(&mut router, &sink, false);
    assert_eq!(press(&mut router, function_key(DOWN)).2.highlight, 1);
}

#[test]
fn matrix_rows_use_the_direction_returned_by_the_displayed_frame() {
    let mut router = router_with(RouterConfig {
        layout: LayoutMode::Horizontal,
        vertical_reverse_above: false,
        horizontal_grid: true,
        page_size: 2,
        ..RouterConfig::default()
    });
    let sink = RecordingCandidates::default();
    router.set_candidate_sink(Box::new(sink.clone()));
    let (_, _, initial) = type_letters(&mut router, "ni");
    assert!(
        initial.page_count > 1,
        "matrix navigation needs multiple rows"
    );
    router.handle(ClientMessage::PositionCandidates {
        session: SESSION,
        rect: rect(),
    });
    place(&mut router, &sink, true);
    let (_, _, expanded) = press(&mut router, function_key(UP));
    assert_eq!(expanded.columns, 2);
    assert_eq!(expanded.highlight, 2);
    assert_eq!(press(&mut router, function_key(DOWN)).2.highlight, 0);
}
