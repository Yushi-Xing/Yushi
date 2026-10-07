//! 标准候选 COM 接口的边界、回调排队与生命周期回归。

use super::element::Element;
use super::{Candidates, State};
use qingjian_core::{Candidate, CandidateKind};
use qingjian_platform::protocol::{CandidateAction, Frame};
use std::rc::Rc;
use windows::Win32::UI::TextServices::{
    ITfCandidateListUIElement, ITfCandidateListUIElementBehavior, ITfUIElement,
};
use windows::core::{ComObject, Interface};

pub(in crate::com::candidates) fn frame() -> Frame {
    Frame {
        typed_keys: "kaifa".into(),
        candidates: qingjian_core::CandidateList {
            items: ["环太平洋", "太平洋", "电影"]
                .into_iter()
                .map(|text| Candidate {
                    text: text.into(),
                    kind: CandidateKind::Chinese,
                    syllables: vec![],
                    reading: None,
                    translation: None,
                    aux_code: None,
                })
                .collect(),
        },
        ..Default::default()
    }
}

#[test]
fn host_queries_candidates_and_queues_a_selection_without_reentering_ipc() {
    let state = Rc::new(State::default());
    *state.frame.borrow_mut() = frame();
    *state.pages.borrow_mut() = vec![0];
    let element = ComObject::new(Element::new(state.clone()));
    let list: ITfCandidateListUIElement = element.to_interface();
    let behavior: ITfCandidateListUIElementBehavior = element.to_interface();
    let ui: ITfUIElement = element.to_interface();
    unsafe {
        assert_eq!(list.GetCount().unwrap(), 3);
        assert_eq!(list.GetString(0).unwrap().to_string(), "环太平洋");
        assert!(list.GetString(3).is_err());
        assert!(behavior.SetSelection(u32::MAX).is_err());
        ui.Show(false).unwrap();
        assert!(!ui.IsShown().unwrap().as_bool());
        behavior.SetSelection(2).unwrap();
        behavior.Finalize().unwrap();
    }
    let actions = state.actions.borrow();
    assert_eq!(actions.len(), 2);
    assert_eq!(
        actions[0],
        CandidateAction::Visibility { own_window: false }
    );
    assert_eq!(
        actions[1],
        CandidateAction::Finalize {
            typed_keys: "kaifa".into(),
            page: 0,
            index: 2,
            text: "电影".into()
        }
    );
}

#[test]
fn host_paging_checks_capacity_order_and_candidate_bounds() {
    let state = Rc::new(State::default());
    *state.frame.borrow_mut() = frame();
    *state.pages.borrow_mut() = vec![0];
    let element = ComObject::new(Element::new(state));
    let list: ITfCandidateListUIElement = element.to_interface();
    let behavior: ITfCandidateListUIElementBehavior = element.to_interface();
    let mut count = 0;
    unsafe {
        assert!(list.SetPageIndex(&[1]).is_err());
        assert!(list.SetPageIndex(&[0, 0]).is_err());
        assert!(list.SetPageIndex(&[0, 3]).is_err());
        list.SetPageIndex(&[0, 2]).unwrap();
        assert!(
            (Interface::vtable(&list).GetPageIndex)(
                Interface::as_raw(&list),
                std::ptr::null_mut(),
                2,
                &mut count
            )
            .is_err()
        );
        let mut small = [0];
        assert!(list.GetPageIndex(&mut small, &mut count).is_err());
        assert_eq!(count, 2);
        let mut pages = [9, 9];
        list.GetPageIndex(&mut pages, &mut count).unwrap();
        assert_eq!(pages, [0, 2]);
        behavior.SetSelection(2).unwrap();
        assert_eq!(list.GetCurrentPage().unwrap(), 1);
    }
}

#[test]
fn ended_candidate_ui_cannot_keep_a_stale_finalize_callback() {
    let candidates = Candidates::new();
    candidates.update(&frame(), None).unwrap();
    candidates.state.queue(CandidateAction::Finalize {
        typed_keys: "kaifa".into(),
        page: 0,
        index: 0,
        text: "环太平洋".into(),
    });
    candidates.deactivate();
    assert!(candidates.take_actions().is_empty());
    assert!(candidates.state.frame.borrow().is_empty());
}

mod manager;
mod search;

#[test]
fn uiless_without_a_ui_manager_does_not_show_an_independent_window() {
    let candidates = Candidates::new();
    candidates.set_uiless(true);
    candidates.update(&frame(), None).unwrap();
    assert!(!candidates.state.shown.get());
    assert_eq!(
        candidates.take_visibility(),
        Some(CandidateAction::Visibility { own_window: false })
    );
    for _ in 0..50 {
        candidates.update(&frame(), None).unwrap();
    }
    assert!(candidates.take_actions().is_empty());
}

#[test]
fn hosted_visibility_never_reopens_the_server_window() {
    let state = State::default();
    state.hosted.set(true);
    state.queue(CandidateAction::Visibility { own_window: true });
    assert_eq!(
        *state.actions.borrow(),
        [CandidateAction::Visibility { own_window: false }]
    );
    state.hosted.set(false);
    state.queue(CandidateAction::Visibility { own_window: true });
    assert_eq!(
        *state.actions.borrow(),
        [CandidateAction::Visibility { own_window: true }]
    );
}

#[test]
fn early_visibility_flush_preserves_a_pending_selection() {
    let candidates = Candidates::new();
    candidates
        .state
        .queue(CandidateAction::Visibility { own_window: false });
    candidates.state.queue(CandidateAction::Finalize {
        page: 0,
        index: 0,
        text: "环太平洋".into(),
        typed_keys: "kaifa".into(),
    });
    assert_eq!(
        candidates.take_visibility(),
        Some(CandidateAction::Visibility { own_window: false })
    );
    assert!(matches!(
        candidates.take_actions().as_slice(),
        [CandidateAction::Finalize { .. }]
    ));
}
