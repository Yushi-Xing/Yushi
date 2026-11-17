# Win11 任务栏搜索与开始菜单候选遮挡排查

用户反馈：任务栏搜索 / 开始菜单会遮住输入法候选窗。核对范围是本仓库 `f5564e4d` 与合入上游 `94ed1917` 后的显示链路；当前环境是 Linux，没有在用户的 Win11 会话中复现，尚不能把某个失败分支认定为此次故障的唯一原因。

## 已确认的代码事实

| 路径 | 当前行为 | 对遮挡问题的影响 |
| --- | --- | --- |
| [`server/src/ui/candidates/mod.rs`](../../apps/windows/server/src/ui/candidates/mod.rs) | Server 独立窗口使用 `WS_EX_TOPMOST`，每次实际更新时还会调用 `SetWindowPos(HWND_TOPMOST)` | 已经做了普通置顶；不能把故障简单归因于漏设 topmost |
| [`windows-preview.yml`](../../.github/workflows/windows-preview.yml)、[`release.yml`](../../.github/workflows/release.yml) | Windows 发布构建显式设置 `QINGJIAN_UIACCESS=0` | 不能假定安装到 Program Files 后就获得 UIAccess |
| [`candidates/hosted/mod.rs::search_host`](../../apps/windows/tsf/src/com/candidates/hosted/mod.rs) | 仅匹配 `SearchHost.exe` / `SearchApp.exe` | 如果实际输入宿主是 `StartMenuExperienceHost.exe`、`explorer.exe` 等，专用子窗口回退不会启用；开始菜单点击搜索后也可能转到 SearchHost，必须按实际宿主区分 |
| 同文件 `parent` | 首先要求前台 HWND 属于当前 DLL 宿主进程，条件不满足就返回；随后才检查文本上下文 | 前台容器与输入宿主分属不同进程时，即使上下文窗口有效，也走不到上下文的回退分支；这是需实测的失败条件 |
| [`candidates/mod.rs::sync_hosted`](../../apps/windows/tsf/src/com/candidates/mod.rs) | 等待光标矩形；服从 TSF 显示协商；创建失败后本段不再尝试，恢复 Server 窗口 | 这几种情况都可能没有宿主内候选窗；创建失败有日志，缺少同进程前台窗口或光标矩形则没有专门日志 |
| [`dispatch/candidates/mod.rs`](../../apps/windows/server/src/dispatch/candidates/mod.rs) | 符号 / 翻译面板优先交给 Server 的 `PanelWindow`，不经过普通候选的宿主回退 | 普通候选与符号 / 翻译面板的遮挡需要分别验收 |

此前在 Windows `10.0.26200.0` 上的仓库实测记录已经确认：系统搜索可以盖住独立 topmost 窗口，外部进程直接建立搜索子窗口还曾返回错误 5。随后实现了 TSF 宿主内子窗口，但记录明确没有宣称用户的真实搜索场景已验收通过，见 [2026-10-07 记录](windows-search-hosted-2026-10-07.md)。此次同步继续保留该实现，没有把旧的“待验收”改成“已修复”。

截至 2026-11-05，上游的 [#78](https://github.com/qingjian-team/qingjian/issues/78) 仍为开放状态。2026-10-20 的维护讨论只确认重新置顶在 Windows 10 有效；[#323 的后续反馈](https://github.com/qingjian-team/qingjian/issues/323#issuecomment-6364257875) 明确说明 Windows 11 26H2、青简 0.3.1 仍能按数字选词但看不见候选，维护者于 10-27 确认继续跟踪。11-04 的 [#873](https://github.com/qingjian-team/qingjian/issues/873) 又报告 Windows 10、0.3.2 仍复现。两者关闭原因均为重复归并，不是修复完成。这些报告支持显示链路问题的判断，但不能替代对本 fork 和用户当前机器的验证。

## 同步时核对的接线

- 上游把帧构建移到 `dispatch/composed/frame.rs`，本仓库的 `Frame.typed_keys` 与辅码版本 7 兼容门槛随之迁移，保持宿主点击、翻页与迟到回调的校验。
- TSF 按键与轮询仍通过 `Candidates::update` 进行 UIElement 协商和宿主内显示；在调用宿主 COM 前释放引擎借用。
- 轮询同时保留宿主候选操作队列和上游 Shift 定时处理；上游的翻译面板状态通过 `follow_frame` 同步。

这些是合并兼容性处理，不等于 Win11 遮挡问题的真机修复。

合并提交为 `548ba087`。Rust 1.96.0 下，`cargo fmt --all --check`、Linux workspace 严格 Clippy 均通过；workspace 测试（排除 macOS 壳）合计 1,726 通过、0 失败、6 忽略。Windows GNU 目标对 Server 与 TSF 的 `--all-targets` 严格 Clippy 通过，只做交叉编译检查，没有执行 Windows 原生窗口测试。CLI 样例验证全拼 `nihao` / `kaifa`，以及小鹤混输 `kaifa` / `kdfa` / `kaifh` 的首选分别为「你好」「开发」「开发」「开发」「开放」。

## 同机定位方法

1. 升级后先注销再登录，让已经运行的系统宿主加载新 DLL。单纯在输入法之间切换不会卸载旧 DLL。
2. 分别从任务栏搜索和 Windows 键打开的开始菜单输入 `nihao`；记录哪个入口、青简版本、`winver` 的系统内部版本、屏幕缩放，以及普通候选还是符号 / 翻译面板被遮住。记事本用同样输入作对照。
3. 在 `%LOCALAPPDATA%\Qingjian\logs\tsf.<日期>.log` 对应时刻查找：
   - `候选显示协商 own_ui=... search_host=...`：`search_host=false` 表示未进入专用回退；`own_ui=false` 表示宿主要求接管绘制。
   - `搜索候选使用宿主内子窗口`：成功进入回退。
   - `搜索宿主内候选回退失败，本段不重试`：按随后的错误码定位创建 / 更新失败。
   - `候选显示协商失败`：协商失败后走普通窗口回退。
4. `search_host=true`、`own_ui=true` 且没有成功或失败日志时，优先检查 `parent` 的同进程条件和光标矩形是否送达。日志含 PID，可在任务管理器“详细信息”对照宿主进程。日志不足时先增加只记录进程、窗口关系和错误码的诊断，不记录输入内容。

确认具体失败分支后，在同一 Win11 场景验证修复：候选可见、键盘与鼠标选词、翻页、Esc、焦点切换、关闭再打开，以及不同缩放。只有这些通过才结束该问题；Linux 单元测试和 Windows 交叉类型检查不能替代此验收。

## Windows API 边界

TSF 的 `pbShow` 决定由宿主还是输入法绘制，即使线程是 UI-less 也必须遵守协商结果，不能强制绕过宿主显示决定，见 [Microsoft：UILess Mode Overview](https://learn.microsoft.com/en-us/windows/win32/tsf/uiless-mode-overview)。

UIAccess 需要满足签名、受保护安装位置和 manifest 等要求，不能只把 `QINGJIAN_UIACCESS` 改成 `1`；它也不是对安全桌面访问的通行证。具体要求见 [Microsoft：Security Considerations for Assistive Technologies](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-securityoverview)。
