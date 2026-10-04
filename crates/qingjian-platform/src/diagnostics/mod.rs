//! 诊断导出：仅导出明确允许的配置摘要，不携带原文、注释或用户短语。

mod export;

pub use export::DiagnosticExport;

use toml_edit::{DocumentMut, value};

use crate::{Config, Scheme};

/// 配置解析失败时调用方必须省略原文件，不能回退到原文。
pub fn config_summary(source: &str) -> Result<String, toml::de::Error> {
    let config: Config = toml::from_str(source)?;
    let mut output = DocumentMut::new();
    for section in ["general", "predict", "model", "update"] {
        output[section] = toml_edit::table();
    }
    let general = &config.general;
    // 不调用会记录原始未知字符串的 scheme()；旧配置也只输出已知方案名。
    let scheme = if !general.scheme.trim().is_empty() {
        general.scheme.trim().parse::<Scheme>().ok()
    } else if general.zhuyin == Some(true) {
        Some(Scheme::Zhuyin)
    } else if let Some(legacy) = general
        .shuangpin
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        legacy
            .parse::<qingjian_core::ShuangpinScheme>()
            .ok()
            .map(Scheme::Shuangpin)
    } else {
        Some(Scheme::Pinyin)
    };
    output["general"]["scheme"] = value(scheme.map_or("unrecognized", Scheme::key));
    output["general"]["page_size"] = value(general.page_size() as i64);
    output["general"]["theme"] = value(general.theme.key());
    output["general"]["layout"] = value(general.layout.key());
    output["general"]["preedit"] = value(general.preedit.key());
    output["general"]["english_mode"] = value(general.english_mode);
    output["general"]["learning"] = value(general.learning);
    output["general"]["input_log"] = value(general.input_log);
    output["predict"]["enabled"] = value(config.predict.enabled);
    output["model"]["enabled"] = value(config.model.enabled);
    output["update"]["check"] = value(config.update.check);
    output["update"]["channel"] = value(config.update.channel.key());
    Ok(output.to_string())
}

/// PowerShell 单引号字符串：路径中的变量与反引号始终按字面量处理。
pub fn powershell_literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

#[cfg(test)]
mod tests;
