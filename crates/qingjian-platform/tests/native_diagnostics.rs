//! Windows PowerShell 5.1 原生打包验证；WSL 可通过共享盘运行，显式启用以免依赖宿主。

use std::fs;
use std::path::Path;
use std::process::Command;

use qingjian_platform::diagnostics::{DiagnosticExport, powershell_literal};

fn native_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(windows) {
        text.into_owned()
    } else {
        let rest = text
            .strip_prefix("/mnt/")
            .expect("WSL 测试暂存目录必须在 Windows 共享盘");
        let (drive, tail) = rest.split_once('/').unwrap();
        format!(
            "{}:\\{}",
            drive.to_ascii_uppercase(),
            tail.replace('/', "\\")
        )
    }
}

#[test]
#[ignore = "需要 Windows PowerShell；WSL 中还需 Windows 共享盘与互操作权限"]
fn powershell_archive_handles_literal_paths_and_contains_no_raw_config() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/native-diagnostics");
    fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let fixture = tempfile::Builder::new()
        .prefix("中文 O'Brien $x`z [1] ")
        .tempdir_in(root)
        .unwrap();
    let logs = fixture.path().join("logs");
    fs::create_dir(&logs).unwrap();
    let config = fixture.path().join("config.toml");
    fs::write(&config, "[predict]\napi_key='sk-native-fixture-abcdefgh'\nbase_url='https://user:secret@example.test'\n").unwrap();
    fs::write(
        logs.join("server.2026-10-04.log"),
        "中文故障 sk-native-fixture-abcdefgh\n",
    )
    .unwrap();
    let export = DiagnosticExport::prepare(&logs, Some(&config)).unwrap();
    // 让 Windows 与 WSL 共用暂存内容；保留生成脚本，只替换传输中的路径写法。
    let staged = fixture.path().join("stage");
    fs::create_dir(&staged).unwrap();
    for entry in fs::read_dir(export.path()).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), staged.join(entry.file_name())).unwrap();
    }
    let script_path = export.write_script("diagnostics.zip").unwrap();
    let script = fs::read_to_string(script_path).unwrap().replace(
        &powershell_literal(&export.path().to_string_lossy()),
        &powershell_literal(&native_path(&staged)),
    );
    let runner = fixture.path().join("export.ps1");
    fs::write(&runner, script).unwrap();
    let executable = std::env::var("QINGJIAN_TEST_POWERSHELL").unwrap_or_else(|_| {
        if cfg!(windows) {
            "powershell.exe".into()
        } else {
            "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe".into()
        }
    });
    let result = Command::new(&executable)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(native_path(&runner))
        .arg("-DestinationDirectory")
        .arg(native_path(fixture.path()))
        .arg("-NoReveal")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(fixture.path().join("diagnostics.zip").is_file());
    let validation = format!(
        "$ErrorActionPreference = 'Stop'; Add-Type -AssemblyName System.IO.Compression.FileSystem; \
         $zip = [IO.Compression.ZipFile]::OpenRead({}); \
         try {{ if ($zip.Entries.Count -ne 3) {{ throw 'unexpected file count' }}; \
         foreach ($entry in $zip.Entries) {{ \
           if ($entry.Name -eq 'config.toml' -or $entry.Name -eq 'export.ps1') {{ throw 'unsafe entry' }}; \
           $reader = New-Object IO.StreamReader($entry.Open()); \
           try {{ $text = $reader.ReadToEnd(); if ($text.Contains('sk-native-fixture') -or $text.Contains('user:secret')) {{ throw 'secret leaked' }} }} \
           finally {{ $reader.Dispose() }} \
         }} }} finally {{ $zip.Dispose() }}",
        powershell_literal(&native_path(&fixture.path().join("diagnostics.zip")))
    );
    let result = Command::new(executable)
        .args(["-NoProfile", "-NonInteractive", "-Command", &validation])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
