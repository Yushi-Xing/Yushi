//! 完全相同输入的最近选择：累计次数保持原格式，额外侧车保存最新偏好。

use std::io::Write;
use std::path::{Path, PathBuf};

use qingjian_core::storage::write_atomic;

use super::{FrequencyLearner, MAX_CHOICE_ENTRIES, RAW_MARK, data_lines};
use crate::error::LearningError;

const UNDO_LIMIT: usize = 64;

impl FrequencyLearner {
    pub(super) fn preferences_path(frequency_path: &Path) -> PathBuf {
        frequency_path.with_file_name("user-preferences.tsv")
    }

    pub(super) fn remember_preference(&mut self, input: &str, text: &str) {
        if text == RAW_MARK {
            return;
        }
        if self.preferences.len() >= MAX_CHOICE_ENTRIES && !self.preferences.contains_key(input) {
            // 额度满时继续更新既有输入，新增输入仍按累计次数排序，避免每键扫描整表。
            return;
        }
        let previous = self.preferences.insert(input.to_owned(), text.to_owned());
        if self.preference_undo.len() == UNDO_LIMIT {
            self.preference_undo.pop_front();
        }
        self.preference_undo
            .push_back((input.to_owned(), text.to_owned(), previous));
        self.preferences_dirty = true;
    }

    pub(super) fn retract_preference(&mut self, input: &str, text: &str) {
        let Some(at) = self
            .preference_undo
            .iter()
            .rposition(|(key, chosen, _)| key == input && chosen == text)
        else {
            return;
        };
        let (_, _, previous) = self
            .preference_undo
            .remove(at)
            .expect("known preference undo");
        if self
            .preferences
            .get(input)
            .is_some_and(|chosen| chosen == text)
        {
            match previous {
                Some(previous) => {
                    self.preferences.insert(input.to_owned(), previous);
                }
                None => {
                    self.preferences.remove(input);
                }
            }
            self.preferences_dirty = true;
        }
    }

    pub(super) fn load_preferences(&mut self, source: &str) -> usize {
        let mut skipped = 0;
        for line in data_lines(source) {
            let Some((input, text)) = line.split_once('\t') else {
                skipped += 1;
                continue;
            };
            if input.is_empty()
                || text.is_empty()
                || text.contains('\t')
                || text == RAW_MARK
                || self.preferences.len() >= MAX_CHOICE_ENTRIES
            {
                skipped += 1;
                continue;
            }
            self.preferences.insert(input.to_owned(), text.to_owned());
        }
        skipped
    }

    pub(super) fn save_preferences_to(&mut self, path: &Path) -> Result<(), LearningError> {
        write_atomic(path, |file| {
            writeln!(file, "# 青简同输入最近选择：输入串\t词")?;
            for (input, text) in &self.preferences {
                writeln!(file, "{input}\t{text}")?;
            }
            Ok(())
        })?;
        self.preferences_dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::FrequencyLearner;
    use qingjian_core::Learner;

    #[test]
    fn repeated_selection_can_be_undone_without_erasing_the_previous_preference() {
        let mut learner = FrequencyLearner::default();
        learner.record_choice("m", "mei", "没");
        learner.record_choice("m", "ma", "吗");
        learner.record_choice("m", "ma", "吗");
        learner.unrecord_choice("m", "ma", "吗");
        assert_eq!(learner.preferred_choice("m"), Some("吗"));
        learner.unrecord_choice("m", "ma", "吗");
        assert_eq!(learner.preferred_choice("m"), Some("没"));
        assert_eq!(learner.preferred_choice("ma"), None);
    }

    #[test]
    fn retracting_an_old_choice_does_not_override_its_replacement() {
        let mut learner = FrequencyLearner::default();
        learner.record_choice("m", "mei", "没");
        learner.record_choice("m", "ma", "吗");
        learner.unrecord_choice("m", "mei", "没");
        assert_eq!(learner.preferred_choice("m"), Some("吗"));
        learner.forget("吗");
        assert_eq!(learner.preferred_choice("m"), None);
    }

    #[test]
    fn preference_only_changes_are_marked_for_saving_and_included_in_exports() {
        let mut learner = FrequencyLearner::default();
        learner.load_preferences("m\t吗\n");
        assert!(!learner.has_unsaved());
        learner.forget("吗");
        assert!(learner.has_unsaved());
        assert!(
            FrequencyLearner::files(std::path::Path::new("user.tsv"))
                .iter()
                .any(|p| p.ends_with("user-preferences.tsv"))
        );
    }
}
