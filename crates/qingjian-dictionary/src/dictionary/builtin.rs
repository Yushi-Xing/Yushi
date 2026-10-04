//! 独立于产品数据归档的基础读音补丁，编译时嵌入，保证旧数据包也能使用。

use super::Dictionary;
use crate::DictionaryError;

impl Dictionary {
    pub fn builtin_patch() -> Result<Self, DictionaryError> {
        Self::parse(include_str!("../../../../assets/lexicon/patches.tsv"))
    }
}

#[cfg(test)]
mod tests {
    use super::Dictionary;

    #[test]
    fn common_en_reading_is_available_without_external_data() {
        let dictionary = Dictionary::builtin_patch().unwrap();
        let hits = dictionary.lookup(&["en"], false);
        assert!(hits.iter().any(|hit| hit.text == "嗯"));
        assert!(dictionary.lookup(&["ni", "hao"], false).is_empty());
    }
}
