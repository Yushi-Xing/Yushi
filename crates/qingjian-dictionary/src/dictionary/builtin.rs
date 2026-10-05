//! 独立于产品数据归档的读音补丁与常用补充词库，编译时嵌入，热加载时保留。

use super::Dictionary;
use crate::DictionaryError;

impl Dictionary {
    pub fn builtin_patch() -> Result<Self, DictionaryError> {
        Self::parse(concat!(
            include_str!("../../../../assets/lexicon/patches.tsv"),
            "\n",
            include_str!("../../../../assets/lexicon/supplement/dict.tsv")
        ))
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

    #[test]
    fn daily_brands_and_supplies_are_available_without_optional_domains() {
        let dictionary = Dictionary::builtin_patch().unwrap();
        for (text, syllables) in [
            ("双汇", vec!["shuang", "hui"]),
            ("双汇火腿肠", vec!["shuang", "hui", "huo", "tui", "chang"]),
            ("充电宝", vec!["chong", "dian", "bao"]),
            ("抽纸", vec!["chou", "zhi"]),
            ("电子发票", vec!["dian", "zi", "fa", "piao"]),
            ("环境变量", vec!["huan", "jing", "bian", "liang"]),
        ] {
            assert!(
                dictionary
                    .lookup(&syllables, false)
                    .iter()
                    .any(|hit| hit.text == text),
                "{text}"
            );
        }
        assert!(
            !dictionary
                .lookup(&["shuang", "hui", "huo", "tui", "zhang"], false)
                .iter()
                .any(|hit| hit.text == "双汇火腿肠")
        );
        assert!(
            !dictionary
                .lookup(&["chong", "dian", "bao"], false)
                .iter()
                .any(|hit| hit.text == "重电宝")
        );
    }
}
