//! 按 Unicode 字符计编辑距离，避免 UTF-8 字节和候选长度带来的统计偏差。

pub(super) fn edit_distance(expected: &str, actual: &str) -> usize {
    let actual: Vec<char> = actual.chars().collect();
    let mut row: Vec<usize> = (0..=actual.len()).collect();
    for (i, a) in expected.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, b) in actual.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(a != *b))
                .min(above + 1)
                .min(row[j] + 1);
            diagonal = above;
        }
    }
    row[actual.len()]
}

#[cfg(test)]
mod tests {
    use super::edit_distance;

    #[test]
    fn counts_unicode_insertions_deletions_and_substitutions() {
        for (expected, actual, errors) in [
            ("", "", 0),
            ("森林碳储量", "森林碳储量", 0),
            ("森林碳储量", "森碳储量", 1),
            ("森林碳储量", "森林的碳储量", 1),
            ("森林碳储量", "森林探储亮", 2),
            ("中国🦀", "中🦀国", 2),
            ("你好", "", 2),
            ("", "你好吗", 3),
        ] {
            assert_eq!(
                edit_distance(expected, actual),
                errors,
                "{expected} / {actual}"
            );
            assert_eq!(edit_distance(actual, expected), errors);
        }
    }
}
