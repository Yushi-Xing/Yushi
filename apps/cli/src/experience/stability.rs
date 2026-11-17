//! 重复查询的候选稳定性：时间快捷项会随时钟刷新，词语、数量与槽位仍须不变。

use qingjian_core::{Candidate, CandidateKind};

pub(super) fn same(input: &str, first: &[Candidate], repeated: &[Candidate]) -> bool {
    let clock = matches!(input, "sj" | "shijian");
    first.len() == repeated.len()
        && first.iter().zip(repeated).all(|(before, after)| {
            before.kind == after.kind
                && (before.text == after.text || (clock && before.kind == CandidateKind::Shortcut))
        })
}

#[cfg(test)]
mod tests {
    use super::same;
    use qingjian_core::{Candidate, CandidateKind};

    fn item(text: &str, kind: CandidateKind) -> Candidate {
        Candidate {
            text: text.to_owned(),
            kind,
            syllables: Vec::new(),
            reading: None,
            translation: None,
            aux_code: None,
        }
    }

    #[test]
    fn time_refresh_keeps_the_same_slots_and_words() {
        let first = [
            item("时间", CandidateKind::Chinese),
            item("23:59:59.999", CandidateKind::Shortcut),
        ];
        let repeated = [
            item("时间", CandidateKind::Chinese),
            item("00:00:00.001", CandidateKind::Shortcut),
        ];
        assert!(same("sj", &first, &repeated));
        assert!(same("shijian", &first, &repeated));
        assert!(!same("sj", &first, &repeated[..1]));
        assert!(!same(
            "sj",
            &first,
            &[repeated[1].clone(), repeated[0].clone()]
        ));
    }

    #[test]
    fn clock_refresh_does_not_hide_changed_word_order() {
        let first = [
            item("世纪", CandidateKind::Chinese),
            item("时间", CandidateKind::Chinese),
        ];
        assert!(!same("sj", &first, &[first[1].clone(), first[0].clone()]));
    }

    #[test]
    fn other_shortcuts_and_candidate_kinds_must_stay_exact() {
        let first = [item("3", CandidateKind::Shortcut)];
        assert!(!same("v1+2", &first, &[item("4", CandidateKind::Shortcut)]));
        assert!(!same("sj", &first, &[item("3", CandidateKind::Chinese)]));
        assert!(same("v1+2", &first, &first));
    }
}
