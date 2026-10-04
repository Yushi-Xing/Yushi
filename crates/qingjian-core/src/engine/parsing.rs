//! 拼音切分的配置入口：显式模糊音先参与切分，再由词库匹配，避免被纠错覆盖。

use super::{Engine, segment_longest_prefix};
use crate::parser::{self, ParseError, Segmentation};

impl Engine {
    pub(super) fn segment_phonetic<'a>(
        &self,
        text: &'a str,
    ) -> Result<(Vec<Segmentation>, &'a str), ParseError> {
        if !self.fuzzy.any() {
            return segment_longest_prefix(text);
        }
        let segment = |input: &str| {
            parser::segment_with(input, |token| self.fuzzy.accepts_nonstandard(token))
        };
        match segment(text) {
            Ok(segmentations) => Ok((segmentations, "")),
            Err(ParseError::NoSegmentation) => (1..text.len())
                .rev()
                .find_map(|end| segment(&text[..end]).ok().map(|s| (s, &text[end..])))
                .ok_or(ParseError::NoSegmentation),
            Err(error) => Err(error),
        }
    }
}
