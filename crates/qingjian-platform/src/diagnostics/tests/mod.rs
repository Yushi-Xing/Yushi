//! 诊断摘要的敏感数据负例、暂存隔离、清理及导出脚本。

use std::fs;

use super::{DiagnosticExport, config_summary, powershell_literal};

#[test]
fn summary_keeps_safe_settings_and_omits_all_user_text() {
    let source = r#"
# secret-in-comment
custom_phrases = [{ code = "mm", text = "secret-personal-phrase", position = 1 }]
[general]
scheme = "pinyin"
learning_language = "secret-language"
page_size = 5
theme = "dark"
layout = "horizontal"
preedit = "window"
input_log = false
[predict]
enabled = true
api_key = "sk-diagnostic-test-abcdefgh"
api_key_env = "secret-env-name"
base_url = "https://user:secret-url@example.test/v1?token=secret-query"
model = "secret-model"
[apps]
english_candidates_off = ["secret-app.exe"]
[unknown]
credentials = { api_key = "secret-inline", nested = ["secret-array"] }
multiline = '''secret-multiline
second-line'''
"#;
    let summary = config_summary(source).unwrap();
    assert!(!summary.contains("secret"), "{summary}");
    assert!(!summary.contains("sk-diagnostic"));
    let parsed: toml::Value = toml::from_str(&summary).unwrap();
    assert_eq!(parsed["general"]["theme"].as_str(), Some("dark"));
    assert_eq!(parsed["general"]["page_size"].as_integer(), Some(5));
    assert_eq!(parsed["general"]["input_log"].as_bool(), Some(false));
    assert_eq!(parsed["predict"]["enabled"].as_bool(), Some(true));
    assert_eq!(parsed.as_table().unwrap().len(), 4);
    assert_eq!(parsed["predict"].as_table().unwrap().len(), 1);
}

#[test]
fn inline_sections_unknown_scheme_and_empty_file_do_not_leak() {
    for source in [
        "",
        "predict = {api_key='secret-inline', enabled=true}\n",
        "[general]\nscheme='secret-scheme' # secret-comment\n",
    ] {
        let summary = config_summary(source).unwrap();
        assert!(!summary.contains("secret"));
        assert!(toml::from_str::<toml::Value>(&summary).is_ok());
    }
    for source in [
        "[predict",
        "[general]\npage_size='secret-bad-type'",
        "[predict]\napi_key='unterminated",
    ] {
        assert!(config_summary(source).is_err());
    }
}

#[test]
fn staging_preserves_sources_filters_files_and_masks_current_key() {
    let source = tempfile::tempdir().unwrap();
    let logs = source.path().join("logs");
    fs::create_dir(&logs).unwrap();
    let config = source.path().join("config.toml");
    let original = "[predict]\napi_key='sk-export-current-123456789'\n";
    fs::write(&config, original).unwrap();
    fs::write(
        logs.join("server.2026-10-04.log"),
        "失败 sk-export-current-123456789\n",
    )
    .unwrap();
    for name in [
        ".env",
        "input-log.jsonl",
        "personal.txt",
        "server.999999999999999999-01-01.log",
    ] {
        fs::write(logs.join(name), "PRIVATE").unwrap();
    }
    fs::create_dir(logs.join("tsf.2026-10-04.log")).unwrap();
    let export = DiagnosticExport::prepare(&logs, Some(&config)).unwrap();
    assert_eq!(
        fs::read_to_string(export.path().join("server.2026-10-04.log")).unwrap(),
        "失败 ***\n"
    );
    let names: Vec<_> = fs::read_dir(export.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 3);
    assert_eq!(fs::read_to_string(&config).unwrap(), original);
    assert_eq!(
        fs::read_to_string(logs.join("server.2026-10-04.log")).unwrap(),
        "失败 sk-export-current-123456789\n"
    );
}

#[test]
fn unreadable_broken_oversized_and_missing_configs_never_copy_raw_file() {
    let source = tempfile::tempdir().unwrap();
    let config = source.path().join("config.toml");
    for text in [
        "[predict\nsecret-broken".to_owned(),
        "# secret-large\n".repeat(100_000),
    ] {
        fs::write(&config, text).unwrap();
        let export = DiagnosticExport::prepare(source.path(), Some(&config)).unwrap();
        let info =
            fs::read_to_string(export.path().join("config-summary-unavailable.txt")).unwrap();
        assert!(!info.contains("secret"));
        assert!(!export.path().join("config.toml").exists());
    }
    fs::remove_file(&config).unwrap();
    let export = DiagnosticExport::prepare(source.path(), Some(&config)).unwrap();
    assert!(
        export
            .path()
            .join("config-summary-unavailable.txt")
            .is_file()
    );
}

#[test]
fn independent_staging_directories_clean_on_drop_and_script_has_bom() {
    let source = tempfile::tempdir().unwrap();
    let first = DiagnosticExport::prepare(source.path(), None).unwrap();
    let second = DiagnosticExport::prepare(source.path(), None).unwrap();
    assert_ne!(first.path(), second.path());
    let path = first.path().to_owned();
    let script = first.write_script("中文 ' $ ` [x].zip").unwrap();
    let bytes = fs::read(script).unwrap();
    assert!(bytes.starts_with(&[0xef, 0xbb, 0xbf]));
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("[IO.Compression.ZipFile]::CreateFromDirectory($source, $zip)"));
    assert!(text.contains("'中文 '' $ ` [x].zip'"));
    assert!(text.contains("$ErrorActionPreference = 'Stop'"));
    drop(first);
    assert!(!path.exists());
    assert!(second.path().is_dir());
}

#[test]
fn paths_are_powershell_literals_not_executable_fragments() {
    assert_eq!(
        powershell_literal("C:\\中文 空格\\O'Brien\\$x`z[1]"),
        "'C:\\中文 空格\\O''Brien\\$x`z[1]'"
    );
    assert_eq!(
        powershell_literal("'; Write-Output PWNED; '"),
        "'''; Write-Output PWNED; '''"
    );
}

#[cfg(unix)]
#[test]
fn log_symlink_does_not_copy_external_user_file() {
    let source = tempfile::tempdir().unwrap();
    let private = source.path().join("private.txt");
    fs::write(&private, "PRIVATE").unwrap();
    std::os::unix::fs::symlink(&private, source.path().join("server.2026-10-04.log")).unwrap();
    let export = DiagnosticExport::prepare(source.path(), None).unwrap();
    assert!(!export.path().join("server.2026-10-04.log").exists());
}

mod limits;

#[test]
fn legacy_scheme_summary_is_canonical_without_unknown_user_strings() {
    for (source, expected) in [
        ("[general]\nshuangpin='xiaohe'", "xiaohe"),
        ("[general]\nzhuyin=true", "zhuyin"),
        ("[general]\nscheme=' pinyin '", "pinyin"),
        ("[general]\nshuangpin='secret-unknown'", "unrecognized"),
    ] {
        let summary = config_summary(source).unwrap();
        assert!(!summary.contains("secret"));
        let parsed: toml::Value = toml::from_str(&summary).unwrap();
        assert_eq!(parsed["general"]["scheme"].as_str(), Some(expected));
    }
}
