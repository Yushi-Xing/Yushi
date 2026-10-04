//! 诊断包的独占暂存目录；只读复制运行日志，持有者退出时自动清理。

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::{config_summary, powershell_literal};
use crate::Config;
use crate::logs::{daily, secrets};

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_LOG_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024;
const MAX_LOG_FILES: usize = 32;

pub struct DiagnosticExport {
    directory: TempDir,

    files: PathBuf,
}

impl DiagnosticExport {
    /// 配置不可读或损坏时仅放一条固定说明；日志不递归，不复制软链接及其他用户文件。
    pub fn prepare(logs: &Path, config_path: Option<&Path>) -> io::Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix("qingjian-diagnostics-")
            .tempdir()?;
        let files = directory.path().join("files");
        fs::create_dir(&files)?;
        let export = Self { directory, files };
        if let Some(path) = config_path {
            let summary = read_bounded(path, MAX_CONFIG_BYTES + 1)
                .ok()
                .filter(|bytes| bytes.len() as u64 <= MAX_CONFIG_BYTES)
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .and_then(|source| {
                    let summary = config_summary(&source).ok()?;
                    // 对已存在的运行日志再掩码一次；未知的旧密钥与历史输入不能据此保证清除。
                    if let Ok(config) = toml::from_str::<Config>(&source)
                        && let Some(key) = config.predict.resolve_api_key()
                    {
                        secrets::register(&key);
                    }
                    Some(summary)
                });
            match summary {
                Some(summary) => fs::write(export.path().join("config-summary.toml"), summary)?,
                None => fs::write(
                    export.path().join("config-summary-unavailable.txt"),
                    "配置无法解析或读取，已省略原文件。\n",
                )?,
            }
        }
        let mut paths = Vec::new();
        for entry in fs::read_dir(logs)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            // 长度先限制，避免不可信文件名触发日期转换中的整数溢出。
            if ["server", "tsf", "settings"].iter().any(|prefix| {
                name.len() == prefix.len() + 15 && daily::log_day(name, prefix).is_some()
            }) {
                paths.push(entry.path());
            }
        }
        paths.sort();
        let mut remaining = MAX_TOTAL_BYTES;
        for path in paths.into_iter().rev().take(MAX_LOG_FILES) {
            if remaining == 0 {
                break;
            }
            let bytes = read_bounded(&path, MAX_LOG_BYTES.min(remaining))?;
            remaining -= bytes.len() as u64;
            let text = String::from_utf8_lossy(&bytes);
            fs::write(
                export.path().join(path.file_name().expect("日志文件名")),
                secrets::mask(&text).as_bytes(),
            )?;
        }
        fs::write(
            export.path().join("export-info.txt"),
            "仅含配置摘要与运行日志；每个日志最多 8 MiB，最多 32 个，总读取量最多 32 MiB。\n历史日志可能含输入内容或未知旧密钥，分享前请检查。\n",
        )?;
        Ok(export)
    }

    pub fn path(&self) -> &Path {
        &self.files
    }

    /// 仅压缩暂存目录；UTF-8 BOM 兼容 Windows PowerShell 5.1 的中文路径。
    pub fn write_script(&self, zip_name: &str) -> io::Result<std::path::PathBuf> {
        let script = archive_script(&self.path().to_string_lossy(), zip_name);
        // 脚本在待压缩目录之外，避免把暂存路径也打包进去。
        let script_path = self.directory.path().join("export.ps1");
        fs::write(&script_path, format!("\u{feff}{script}"))?;
        Ok(script_path)
    }
}

pub(super) fn archive_script(directory: &str, zip_name: &str) -> String {
    format!(
        "param([string]$DestinationDirectory = [Environment]::GetFolderPath('Desktop'), [switch]$NoReveal)\n\
         $ErrorActionPreference = 'Stop'\n\
         $source = {}\n\
         $zip = Join-Path $DestinationDirectory {}\n\
         Add-Type -AssemblyName System.IO.Compression.FileSystem\n\
         if ([IO.File]::Exists($zip)) {{ [IO.File]::Delete($zip) }}\n\
         [IO.Compression.ZipFile]::CreateFromDirectory($source, $zip)\n\
         if (-not $NoReveal) {{ explorer.exe \"/select,`\"$zip`\"\" }}\n",
        powershell_literal(directory),
        powershell_literal(zip_name)
    )
}

fn read_bounded(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}
