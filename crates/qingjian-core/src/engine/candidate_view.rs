//! 当前拼音的浏览范围；只裁展示结果，不缩减内部搜索或改动拼音。

use crate::{Candidate, CandidateKind, Engine};

/// 普通浏览限词数；完整列表和拆字选择保留显式入口。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CandidateView {
    Compact,
    #[default]
    All,
    Characters,
}

impl CandidateView {
    /// 普通浏览的整词额度；单字不受此数限制。
    pub const WORD_LIMIT: usize = 27;
}

impl Engine {
    /// 只有提供了“更多候选”入口的壳才启用精简浏览；其他壳默认仍可访问完整列表。
    pub fn set_default_candidate_view(&mut self, view: CandidateView) {
        self.default_candidate_view = view;
        self.set_candidate_view(view);
    }

    pub fn candidate_view(&self) -> CandidateView {
        self.candidate_view
    }

    pub fn set_candidate_view(&mut self, view: CandidateView) {
        self.candidate_view = view;
        self.cancel_rescoring();
    }

    /// 体验评测可比较不同额度；不能越过内部候选上限。
    pub fn set_candidate_word_limit(&mut self, limit: usize) {
        self.candidate_word_limit = limit.clamp(1, super::MAX_CANDIDATES);
        self.cancel_rescoring();
    }

    pub(super) fn apply_candidate_view(&self, items: &mut Vec<Candidate>) {
        if self.english_mode || !self.phonetic || self.expression_mode() || self.question_mode() {
            return;
        }
        let mut words = 0;
        items.retain(|candidate| {
            let chinese = matches!(
                candidate.kind,
                CandidateKind::Chinese
                    | CandidateKind::Sentence
                    | CandidateKind::Generated
                    | CandidateKind::Cloud
            );
            if !chinese {
                return self.candidate_view != CandidateView::Characters;
            }
            let single = candidate.text.chars().count() == 1 && candidate.syllables.len() == 1;
            match self.candidate_view {
                CandidateView::All => true,
                CandidateView::Characters => single,
                CandidateView::Compact if single => true,
                CandidateView::Compact => {
                    words += 1;
                    words <= self.candidate_word_limit
                }
            }
        });
    }
}
