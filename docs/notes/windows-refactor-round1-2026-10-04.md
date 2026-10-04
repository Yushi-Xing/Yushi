# Windows 重构第一轮：2026-10-04

本轮完成安全底座的第一组修改。完整路线和状态见 [任务清单](../plan/windows-refactor-tasks.md)，产品目标见 [重构规格](../plan/windows-focused-refactor.md)。本记录不代表 Win10/Win11 输入宿主已经验收，也不代表审计发现全部修复。

## 改动

- **会话连接隔离**：客户端编号只在当前连接内有效，由传输入口映射到进程内唯一编号；回包还原原编号。所有会话读写必须先登记，每条连接最多 64 个会话；EOF、坏帧和回包失败清理本连接会话。旧 DLL 的一次性 `ImeSwitched` 仅保留状态条通知，不登记会话。后台会话的 Commit 不再清空前台组句。
- **诊断导出**：配置解析后只生成固定允许字段的摘要，省略密钥、地址、任意文本、自定义短语、应用清单、注释与未知字段；损坏／缺失／超大配置只写固定说明，不回退原文件。源文件保持不变。独占随机暂存目录由后台工作线程持有，PowerShell 结束后清理；脚本使用 BOM 与单引号字面量，独立于 ZIP 内容。只复制三类日期运行日志，重新掩码当前已知密钥，单份读取限 8 MiB、最多 32 份、总读取限 32 MiB。
- **运行日志**：移除 TSF 的逐键字符／键码／拼音／上屏内容日志及无上下文错误路径的文本；Server 的 Commit 调试日志也不写文本。这不等于关闭独立的 InputLog 产品功能，后者的默认策略在下一轮处理。历史日志和未知旧密钥不保证自动清除，设置页与用户文档均已说明分享前检查。
- **更新下载**：索引限 2 MiB，分离签名限 1 KiB；声明超限即拒绝，实际响应逐块累计，chunked／无长度响应也受限。原来的签名验证顺序保持不变。

## 测试结果

| 检查 | 实际结果 | 范围 |
|---|---|---|
| 全量适用 workspace 测试 | 666 通过、0 失败、2 默认跳过 | Linux/WSL；排除 macOS 专用壳 |
| 原生 PowerShell ZIP 测试（显式启用） | 1 通过、0 失败 | 本机 Win11 10.0.26200.0；虚构配置和日志，不安装或切换输入法 |
| workspace Clippy | 通过，警告视为错误 | 排除 macOS，包含所有测试目标 |
| Windows 三程序 Clippy | 通过，警告视为错误 | x86_64-pc-windows-gnu，Server/TSF/设置程序，包含测试目标 |
| rustfmt 与 git diff --check | 通过 | 工作区修改 |
| 真实 TSF 命名管道测试 | 只交叉编译检查，未执行 | 用例已加强为两条同编号连接和未登记读取 |
| Win10／Win11 输入框、浏览器、VS Code、微信 | 未执行 | 按用户指定顺序待真机测试 |

相对审计时 641 项基线，新增 **25 项默认回归 + 1 项显式原生打包测试**。默认跳过的两个是模型 batch latency 基准和原生打包测试；后者本轮已单独执行成功，前者未运行。Linux 中 `cfg(windows)` 的管道与 COM 测试不会运行，不能把 666 项全量通过解释为它们已通过。

从实际测试输出提取的统计和通过用例名称见 [测试结果 JSON](windows-refactor-round1-2026-10-04-tests.json)。

回归用例覆盖：

- 所有会话操作在未登记时拒绝；跨连接同编号、猜内部编号、0／u64::MAX、重复开会话、关闭后恢复容量、旧 DLL 单向通知。
- 真帧传输中的越权读取／提交／关闭／改隐私／按键；对前台输入无副作用；EOF、坏 JSON、截断帧、写失败；当前与旧协议的回话顺序。
- 配置正文、注释、inline table、多行文本、未知字段、URL 凭据、自定义短语和应用清单的敏感文本；空／坏／缺失／超大配置；旧方案配置的安全摘要；源文件不改写。
- 暂存目录隔离与销毁，日志软链接排除（Unix 用例）、文件数量与实际读取量上限、当前密钥掩码；ZIP 真正包含三份安全暂存文件、不含原配置与脚本。
- 普通、空、恰好限额和声明超限 HTTP；chunked 累计边界、超限但仍不断流；无长度 EOF；UTF-8 按字节计；404／429／500、截断正文；真实生产限额的恰好边界。

原生测试使用含中文、空格、单引号、美元、反引号、方括号的路径。最初用 `Compress-Archive -LiteralPath` 仍失败；改用系统 .NET ZIP 接口后通过。这个问题是原生执行发现的，字符串断言与 Linux 单元测试没有发现。

## 复跑

在相应工具链与依赖可用的环境中：

```bash
cargo test --workspace --exclude qingjian-macos --locked
cargo clippy --workspace --exclude qingjian-macos --all-targets --locked -- -D warnings
cargo clippy -p qingjian-windows-server -p qingjian-windows-tsf -p qingjian-windows-settings --all-targets --target x86_64-pc-windows-gnu --locked -- -D warnings
cargo test -p qingjian-platform --test native_diagnostics --locked -- --ignored
cargo fmt --all --check
git diff --check
```

WSL 原生打包测试要求仓库在 `/mnt/<盘符>/` 的 Windows 共享盘上，且允许调用 Windows PowerShell；可用 `QINGJIAN_TEST_POWERSHELL` 指定程序路径。所有配置、日志和 ZIP 都在随机测试目录中，退出自动删除；不调用资源管理器、不写用户桌面。Windows 本机运行 Server 测试时，按仓库约定设置 `QINGJIAN_UIACCESS=0`。

本机用已安装 Rust 1.96 与独立 `/tmp` target 目录。Linux OpenSSL 开发头文件及 Windows 交叉编译所需 MinGW 依赖均从 Ubuntu 官方 APT 索引选择，核对 SHA256 后仅解包到 `/tmp`；未用 sudo 替换系统库或持久修改 PATH。Windows PATH 中未发现原生 Cargo/Rustc/MSVC，因此本轮没有构建可安装的 Windows MSVC 产品包。

## 剩余安全与产品工作

- 审计 A1 的管道 Everyone/应用容器授权、账户／登录会话身份与服务端验证仍待修复；会话映射不等于身份认证。全局模式和一次性状态条通知仍需后续认证边界保护。
- A2 的宽泛日志目录 ACL、A4 的过期／未知隐私状态、A5 的连接与队列上限、读写截止时间仍待下一轮。本轮仅落实每连接会话额度与断开清理。
- A3 的 Windows 安全凭据存储仍待实施；本轮修复配置原文导出路径。A7 的更新响应读取限额已修复，云响应限额另行处理。
- Windows 单平台删减、全拼纠错改进、独立英语句译、轻量原生设置和包体测量尚未实施。当前输入按键和词库内容未在本轮改动。

下一轮先处理身份／隐私／恢复边界，再做功能删减及纠错、句译。输入宿主验收顺序固定为：基础 Windows 输入框 → 浏览器 → VS Code → 微信；Win10 和 Win11 分开记录。
