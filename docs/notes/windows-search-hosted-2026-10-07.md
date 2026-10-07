# Windows 搜索候选宿主内回退

## 目标与范围

修复 Win11 系统搜索允许 TIP 显示、但未签名 Server 的独立候选窗被遮住的路径。保留全部双拼、全拼、词库与模型行为；不引入证书、提权、注入、未公开窗口带 API 或修改宿主样式。

## 本机实测与边界

2026-10-07，本机 Windows `10.0.26200.0`，系统搜索前台窗口为 `SearchHost.exe` 的 `Windows.UI.Core.CoreWindow`。短时打开搜索，在固定测试位置创建三种空白诊断窗，再销毁、关闭搜索并恢复前台：

| 显示方式 | 命中测试窗 | 焦点保持 |
| --- | --- | --- |
| 独立 `HWND_TOPMOST` 弹窗 | 否 | 是 |
| SearchHost owned 弹窗 | 否 | 是 |
| 外部进程建立 SearchHost 子窗口 | 创建失败，错误 5 | 是 |

此实验复现了窗口层级问题，未安装或替换用户输入法，也没有读取搜索内容。它不能证明新的 TSF 进程内子窗口已在用户搜索界面验收通过。

## 实现与回归

在本来由系统加载到搜索进程的 TSF DLL 内建立回退子窗口，所属 HWND 必须属于本进程；显示前遵守 TSF 协商，宿主接管时不显示。GDI 仅绘制 Server 已排好的候选帧，不复制引擎。成功创建后隐藏 Server 窗；创建失败保留原回退并记录无内容诊断，本段不反复创建。

测试覆盖父子关系、不抢焦点、DPI、搜索栏位于面板下方、屏幕边缘、过小父窗口、鼠标边界、已绘制帧与过期页校验、重复帧不重绘、父窗口销毁、回调与资源释放、显隐消息不丢选词、UI-less 不可擅自显示。Windows MSVC 运行这些测试，WSL 编译或普通模拟窗口测试不等于搜索栏真机验收。

本机提交前验证：Linux workspace 705 通过、0 失败、3 个显式忽略项；6 个 Python 测试脚本合计 14 通过；fmt、全 workspace 严格 clippy 和 Windows GNU DLL 严格 clippy 通过。新增 Windows 专属用例 10 项，原生运行结果以本版本 CI 为准。

首轮 CI `37643478437` 在 Windows lib-test 编译阶段拦截了两处 `SendMessageW` 参数类型错误；已按 windows 0.62.2 的 `Option<WPARAM>` / `Option<LPARAM>` 签名修正。Linux 全量与 macOS 壳成功，此次失败没有触发标签发布。

第二轮 CI `37644120955` 严格 clippy 成功，但 3 个分层子窗口原生测试创建失败（27 通过、3 失败）。本机同一份 C# 诊断程序对照：缺少支持版本清单时普通子窗口成功、分层子窗口失败；加入 Windows 8 / 10 `supportedOS` 后两种均成功；只激活线程级清单仍然失败。这与 [公开 Win32 使用要求](https://learn.microsoft.com/en-us/windows/win32/winmsg/using-windows) 一致。新增 TSF MSVC 构建脚本给测试 exe 嵌入 ID 1 兼容清单，保持分层测试原样；ID 1 在产品 DLL 中不会覆盖搜索宿主的应用清单，不包含提权、uiAccess 或 DPI 设置。

子窗口与分层显示使用公开 Win32 API，参考 [Windows 窗口特性](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features) 和 [微软分层子窗口示例](https://github.com/microsoft/Windows-classic-samples/blob/main/Samples/DirectCompositionLayeredChildWindow/cpp/DirectComposition_LayeredChildWindow.cpp)。协商遵守 [TSF UI-less 模式](https://learn.microsoft.com/en-us/windows/win32/tsf/uiless-mode-overview) 的 `pbShow`。

## 安装验收

升级后注销再登录，使 SearchHost 重新加载 DLL。保持本地模型关闭，按下列顺序验证：

1. Win11 底部搜索框输入 `huantaipingy`、`shuanghuihuotuichang`，确认候选在搜索面板内完整显示。
2. 输入、停键、退格、左右移动拼音光标、方向键高亮、翻页、数字选词和鼠标选词。
3. Enter 原样上屏、Esc 取消、切换微软拼音、关闭搜索再打开，确认没有残留窗或重复上屏。
4. 100% / 125% / 150% / 200% 缩放、屏幕边缘和多显示器；Win10 搜索用同一流程。
5. 记事本、浏览器、VS Code、微信保持原自绘路径，现有双拼方案继续工作。

只有用户同机搜索场景通过后才将“搜索遮挡”标为完全解决。整句排序、开始菜单与设置应用遮挡不属于本次已验收项。
