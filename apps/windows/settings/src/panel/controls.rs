//! 各页共用的表单零件（标签行、说明小字、整项、页外壳）与打开文件 / 目录、打包日志的小工具。

use std::path::{Path, PathBuf};

use windows_reactor::*;

use super::notice::Notice;
use super::{LABEL_WIDTH, Message, Settings};
use crate::log;

/// 随包资源（相对随包根，如 `data/generated/dicts`），定位逻辑与 Server 共用。
pub(super) fn repo_resource(rel: &str) -> Option<PathBuf> {
    qingjian_platform::resources::bundled_resource(rel)
}

pub(super) fn open_in_editor(path: &Path) {
    if let Err(error) = std::process::Command::new("notepad").arg(path).spawn() {
        log::warn(format!("打开 {} 失败: {error}", path.display()));
    }
}

/// 资源管理器打开目录或网址。
pub(super) fn open_with_explorer(target: &str) {
    if let Err(error) = std::process::Command::new("explorer").arg(target).spawn() {
        log::warn(format!("打开 {target} 失败: {error}"));
    }
}

/// 三个进程共用的日志目录，没有就建出来（Server 没跑过时它还不存在）。
pub(super) fn log_dir() -> Option<PathBuf> {
    let dir = qingjian_platform::dirs::log_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// 后台暂存配置摘要与运行日志，再用 PowerShell 打包到桌面；完成后清理暂存目录。
pub(super) fn export_logs() {
    let Some(logs) = log_dir() else {
        return;
    };
    log::warn("用户导出日志");
    if let Err(error) = std::thread::Builder::new()
        .name("diagnostic-export".into())
        .spawn(move || {
            if let Err(error) = run_log_export(&logs) {
                log::warn(format!("导出日志失败: {error}"));
            }
        })
    {
        log::warn(format!("启动日志导出失败: {error}"));
    }
}

fn run_log_export(logs: &Path) -> std::io::Result<()> {
    use qingjian_platform::diagnostics::DiagnosticExport;
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let config = qingjian_platform::dirs::config_path();
    let export = DiagnosticExport::prepare(logs, config.as_deref())?;
    let zip_name = format!(
        "qingjian-logs-{}.zip",
        jiff::Zoned::now().strftime("%Y-%m-%d")
    );
    let script_path = export.write_script(&zip_name)?;
    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script_path)
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other("PowerShell diagnostic export failed"));
    }
    Ok(())
}

/// 一行设置：固定宽标签 + 控件。
pub(super) fn labeled(label: &str, control: impl Into<View>) -> View {
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(12.0)
        .children([
            TextBlock::new().text(label).width(LABEL_WIDTH).into(),
            control.into(),
        ])
}

/// 灰色小字说明，可换行。
pub(super) fn note(text: &str) -> View {
    TextBlock::new()
        .text(text)
        .text_wrapping(TextWrapping::Wrap)
        .font_size(12.0)
        .opacity(0.6)
        .into()
}

/// 一整项：「标签 + 控件」一行，下接说明（`hint` 为空则不加）。
pub(super) fn field(label: &str, hint: &str, control: impl Into<View>) -> View {
    let row = labeled(label, control);
    if hint.is_empty() {
        row
    } else {
        StackPanel::new().spacing(4.0).children([row, note(hint)])
    }
}

/// 在 `(界面名, 配置写法)` 列表里找 `value` 的下标，找不到取 0。
pub(super) fn index_of(options: &[(&str, &str)], value: &str) -> usize {
    options.iter().position(|(_, v)| *v == value).unwrap_or(0)
}

/// 一行「名称 · N 条 · 随包 / 许可证」，坏文件标出来：词库页与辅码页共用。
pub(super) fn entry_title(
    name: &str,
    entries: usize,
    license: &str,
    builtin: bool,
    broken: bool,
) -> String {
    if broken {
        return format!("{name}（文件损坏）");
    }
    let mut text = format!("{name} · {entries} 条");
    if builtin {
        text.push_str(" · 随包");
    } else if !license.is_empty() {
        text.push_str(&format!(" · {license}"));
    }
    text
}

/// 一行「复选框 + 可选的移除按钮」：词库页与辅码页共用，坏文件禁掉开关。
pub(super) fn check_row(
    stem: &str,
    label: String,
    enabled: bool,
    broken: bool,
    toggle: impl Fn(bool) -> Message + 'static,
    remove: Option<Message>,
    context: &mut ViewContext<Settings>,
) -> KeyedView {
    let check = CheckBox::new()
        .is_checked(enabled)
        .is_enabled(!broken)
        .on_is_checked_changed(context.callback(toggle))
        .content(label);
    let row = match remove {
        Some(message) => StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(12.0)
            .children((
                check,
                Button::new()
                    .on_click(context.message(message))
                    .content("移除"),
            )),
        None => check,
    };
    KeyedView::new(stem.to_owned(), row)
}

/// 页面底部的提示：成功统计一行灰字、失败一行红字，都没有就不画。
pub(super) fn feedback(notice: &Notice) -> View {
    let mut lines: Vec<KeyedView> = Vec::new();
    if let Some(text) = &notice.note {
        lines.push(KeyedView::new("notice-note", note(text)));
    }
    if let Some(text) = &notice.error {
        lines.push(KeyedView::new(
            "notice-error",
            TextBlock::new()
                .text(text)
                .text_wrapping(TextWrapping::Wrap)
                .font_size(12.0)
                .foreground(ThemeBrush::SystemCritical),
        ));
    }
    StackPanel::new().spacing(4.0).keyed_children(lines)
}

/// 一页外壳：可滚动 + 大标题 + 内容。
pub(super) fn page(title: &str, body: impl Into<View>) -> View {
    ScrollViewer::new().content(
        StackPanel::new().spacing(16.0).margin(24.0).children([
            TextBlock::new()
                .text(title)
                .font_size(24.0)
                .font_weight(FontWeight::SEMI_BOLD)
                .into(),
            body.into(),
        ]),
    )
}
