# 安全审计：2026-10-04

本文记录改动前的审计基线与漏洞复现，不随重构覆盖历史证据。后续代码修复、回归结果与剩余问题见 [Windows 重构第一轮记录](windows-refactor-round1-2026-10-04.md)。

审计对象：工作区 `/mnt/d/code/gitlab/Yushi`，提交 `c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e`。审计开始时工作区干净。本文不代表对已发布安装包、作者意图或全部第三方源码的安全认证。

## 结论与范围

**已检查的第一方代码未发现可确认的恶意后门、挖矿、勒索、隐蔽控制端或硬编码窃取密钥行为；发现了多条输入内容和凭据泄露路径，Windows 端尤其需要修复。** 输入日志是仓库明确声明的功能，不能仅凭其存在判定作者恶意；以下问题在于权限、隐私状态和导出边界没有落实。

对 965 个 Git 跟踪文件做了清单和模式扫描，其中 742 个代码／脚本文件约 85,024 行。此数字是自动扫描覆盖量，**不表示逐行人工审查了全部代码**。人工审查集中在 Windows 命名管道与 TSF、Linux Unix socket 与 Fcitx5、macOS IMK、云请求、配置与日志、二进制数据容器、安装与发布脚本、Actions 和依赖锁文件。

跟踪文件中未检测到 ELF、PE 或 Mach-O 可执行文件魔数；常见私钥、GitHub PAT、AWS Access Key 和长 `sk-` 密钥字面量扫描未命中。该扫描不是完整秘密扫描或反病毒扫描，不能排除其他格式的凭据、编码载荷和第三方依赖内的问题。

严重程度按本项目影响与攻击条件评估，未赋予未经验证的 CVSS 分数。标记“代码确认”表示调用链或配置已由源码确认；Windows 真机、跨账户或服务端攻击的端到端复现状态单独说明。

| 编号 | 程度 | 问题 | 主要条件 |
|---|---|---|---|
| A1 | 高 | Windows 管道没有可靠的客户端、服务端和会话身份边界 | 本机不可信进程；跨账户／远程访问取决于系统策略 |
| A2 | 高 | Windows TSF 无条件记录按键与文本，私密模式和日志开关不能阻止；日志向所有应用容器授权 | Windows 正常使用；容器侧访问还取决于父目录与沙箱规则 |
| A3 | 高 | Windows 导出日志包含明文 API 密钥 | 设置云服务密钥后导出并分享日志 |
| A4 | 高 | Windows 翻译选区未检查当前输入范围的私密状态 | 已启用云服务，在新进入的私密输入框触发翻译 |
| A5 | 中 | Windows IPC 无连接资源上限或读取截止时间 | 可连接管道的不可信进程 |
| A6 | 中 | Unix 敏感输入记录和配置没有强制私有权限 | umask 022 且祖先目录允许其他账户遍历 |
| A7 | 中 | 更新响应的大小上限在完整读入之后检查 | 更新源／测试源返回恶意大响应 |

## A1：Windows 管道与会话授权

代码确认：

- [pipe.rs:37](../../apps/windows/server/src/ipc/pipe.rs#L37) 的 `D:(A;;GA;;;WD)...` 向 Everyone 授予 Generic All，并允许两个通用应用容器 SID；管道完整性标记为 Low。
- [pipe.rs:138](../../apps/windows/server/src/ipc/pipe.rs#L138) 未指定 `PIPE_REJECT_REMOTE_CLIENTS`，使用全机固定管道名；未校验客户端的用户、登录会话或进程身份。
- [pipe.rs:169](../../apps/windows/server/src/ipc/pipe.rs#L169) 直接把客户端声明的消息交给同一个 Router，没有每连接的会话映射或所有权校验。
- [message.rs:35](../../apps/windows/server/src/dispatch/message.rs#L35) 把客户端提供的 SessionId 直接放进全局表；`Poll`、`Commit`、`Privacy`、`CloseSession` 等不携带可信的连接主体。
- [session_id](../../apps/windows/tsf/src/com/mod.rs#L86) 使用 Windows 线程 ID，不能当秘密能力凭证。
- [客户端连接](../../apps/windows/tsf/src/client/pipe.rs#L28) 仅按管道名打开，没有验证实际服务端。`FILE_FLAG_FIRST_PIPE_INSTANCE` 只阻止首实例重复创建，不能证明客户端连到的服务端可信，也不能撤销 Everyone 的创建实例权限。

影响：不可信客户端可以声明别人的会话编号，读取当前组句／候选、清空组句、覆盖或关闭会话、修改隐私状态。Everyone 的完全访问还包括管道实例创建权限，使伪服务端／额外实例拦截输入成为需要验证的攻击路径。固定全机名称也使多账户登录互相干扰。**未声称已验证所有 Windows 默认配置下的跨账户或远程利用。** 远程路径还需要 SMB、网络访问策略与防火墙允许。

验证建议：在隔离 Windows 测试账户中，连接 A 创建仅含测试文本的会话；连接 B 不打开该会话，直接请求 A 的 `Poll`／`Commit`。再用不同账户检验 ACL，并单独验证额外管道实例的创建及客户端服务端认证。不要在真实敏感输入会话上测试。

修复方向：按用户与登录会话命名；服务端实例创建权限只给受信任主体，客户端只获得必要读写权限；拒绝远程客户端；每连接分配／映射会话并逐消息检查所有权；结合登录 SID 和进程令牌验证客户端，客户端也验证服务端。AppContainer 兼容不能等价于向 Everyone 授予完全权限。

Microsoft 官方说明：[Named Pipe Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights) 特别指出客户端写权限与实例创建权限的关系，并建议用登录 SID 防止跨登录会话和远程访问。

## A2：Windows TSF 日志绕过隐私与开关

[key_sink.rs:194](../../apps/windows/tsf/src/com/service/key_sink.rs#L194) 无条件调用 `log()`，包含 `event.character`、虚拟键码、修饰键和完整 `preedit`。[document.rs:64](../../apps/windows/tsf/src/com/service/document.rs#L64) 还写入失焦上屏文本。[log.rs:20](../../apps/windows/tsf/src/com/log.rs#L20) 直接追加文件，既不检查 `log_level`／`input_log`，也不检查私密状态。

因此，即使普通引擎日志设置为 info、输入日志关闭，TSF 日志仍然记录经青简处理的内容；应用仅声明 `IS_PRIVATE` 但未禁用键盘时，该路径仍存在。真正设置 keyboard-disabled 的密码框有前置放行路径，本结论**不是“所有密码框都被记录”**。

[main.rs:242](../../apps/windows/server/src/main.rs#L242) 进一步向 `ALL APPLICATION PACKAGES` 和 `ALL RESTRICTED APPLICATION PACKAGES` 授予日志目录继承 Modify 权限，包含读取、写入及删除。容器若能到达该目录，可能读取输入或篡改审计痕迹。父目录和具体沙箱的额外限制必须在 Windows 上验证，不能只由这一条 ACL 推断每个应用都能读取。

仓库[数据与日志说明](../user/help/data-and-logs.md) 声称缺省运行日志不含输入内容，只在详细日志时逐键记录，当前 TSF 实现与该承诺不一致。“打包日志”还会把该目录全部包含，扩大分享时的影响。

修复方向：默认不记录字符、preedit 和上屏文本；确需诊断时采用统一开关并显式检查私密状态。每个宿主／应用容器只写自己的最小日志位置，避免通用容器拥有所有进程日志的读取与修改权限。隐私防护必须覆盖 DLL 自己的日志，不能只在 Engine 的 InputLogger 上实施。

## A3：Windows 日志导出泄露 API 密钥

[component.rs:137](../../apps/windows/settings/src/panel/component.rs#L137) 处理 `CloudApiKey(value)` 时直接 `save("predict", "api_key", value)`，保存目标是 `config.toml`。[controls.rs:48](../../apps/windows/settings/src/panel/controls.rs#L48) 把整个配置文件加入压缩来源，再用 `Compress-Archive` 原样打包，没有脱敏步骤。

完整路径：云服务设置框填写密钥 → `[predict].api_key` 明文落盘 → “打包日志到桌面”包含配置 → 用户按反馈说明分享 ZIP → 收件人获得 API 凭据。无需恶意云服务，且 Windows 正常 UI 即可形成这条路径。源码已确认，尚未在 Windows 上实际创建 ZIP。

修复方向：密钥转入 Windows Credential Manager／DPAPI 保护的独立存储；导出使用解析后的脱敏配置副本，显式删除 `api_key`，不包含 `.env`，并检查其他可能携密的 URL 字段。日志内容也需要单独审查，不能仅删配置键。已分享过这类日志包的用户应撤销并重建相关密钥。

macOS 的诊断导出已有脱敏处理，不能因此认为 Windows 导出也安全。

## A4：Windows 私密选区翻译使用过期状态

输入范围判断仅在[composition::apply](../../apps/windows/tsf/src/com/composition/mod.rs#L49) 的编辑回调中进行，且此回调发生在按键已经交给 Server 之后。

选区翻译走另一条链：

1. [OnPreservedKey](../../apps/windows/tsf/src/com/service/key_sink.rs#L64) 只检查 keyboard-disabled，不能覆盖 `IS_PRIVATE`。
2. Server 根据会话当前状态请求选区；新会话[缺省 private=false](../../apps/windows/server/src/dispatch/message.rs#L39)。
3. [SelectionSession::DoEditSession](../../apps/windows/tsf/src/com/edit/selection.rs#L37) 直接读选中文字并回给 Server，没有调用输入范围检查，也没有先更新隐私状态。
4. [handle_selection](../../apps/windows/server/src/dispatch/translate/mod.rs#L50) 调用 `Engine::request_translation`；Core 虽检查 private，但此时该值可能未反映当前私密输入框。

触发条件：云服务已开启，在新会话／同一客户端新进入的私密输入框中，没有先完成普通组句的隐私上报，就按翻译快捷键。若宿主允许读取选区且未禁键盘，私密原文会被提交给 Predictor。这里并不是 UI 直接把 Engine 标成私密后 Core 仍发送，而是平台壳没有及时传递当前状态。

代码链已确认；宿主具体支持 `IS_PRIVATE`、可读选区和 TSF 回调调度的端到端行为，需要 Windows 真机验证。不得概括为已复现任意浏览器无痕窗口。

修复方向：在读选区的编辑会话中先检查当前 InputScope，私密／密码／PIN 立即拒绝读取和发送；把隐私能力确认作为每个新上下文允许联网和记录的前置条件。普通按键也要处理隐私状态尚未确认的时间窗口。取消隐私边界之前排队的请求需要真实取消机制，单纯增加结果序号只能丢返回结果。

## A5：Windows IPC 资源耗尽

[pipe.rs:99](../../apps/windows/server/src/ipc/pipe.rs#L99) 每个连接新建线程，实例数配置为 `PIPE_UNLIMITED_INSTANCES`，没有应用层连接数或会话数上限。[read_message](../../crates/qingjian-platform/src/protocol/codec.rs#L47) 收到合法长度前缀后先分配整个 body，再阻塞读满；单帧最多 16 MiB，连接读取没有截止时间。

攻击者建立多个连接，分别发送合法的 16 MiB 长度前缀但不发送正文，就可以持续占用线程及内存；大量 OpenSession 也不会在连接结束时自动清理。OS 仍有自身资源限制，不能把 `PIPE_UNLIMITED_INSTANCES` 理解成无穷多个成功实例，但足以在资源耗尽前产生拒绝服务。

修复方向：连接／会话数量上限、首帧及正文读取截止时间、按身份配额、有界任务队列、断线回收；按具体消息设置更小的长度上限。Linux 已有 64 连接上限和有界队列，但缺少读取截止时间和每连接会话数量限制，属于同账户拒绝服务的额外加固项。

## A6：Unix 输入与配置落盘权限

输入日志缺省开启（[GeneralConfig](../../crates/qingjian-platform/src/config/general.rs#L147)）。[InputLog::open](../../crates/qingjian-learning/src/input_log.rs#L38) 没有指定 0600；[普通原子写入](../../crates/qingjian-core/src/storage/mod.rs#L56) 也仅在 private 分支指定权限，配置调用普通分支。macOS／Linux 创建用户数据、配置和日志目录时未强制设为 0700。

在 umask 022 下，新建文件通常为 0644，目录为 0755。**只有祖先目录也允许遍历时，其他本机账户才实际能读取。** 祖先目录为 0700 的系统可以挡住此路径，不能说每台 Unix 机器都泄露。记录可能包含按键、上屏文本、候选、应用信息；手动在配置中放 API 密钥时，配置也成为敏感文件。

修复方向：敏感目录强制 0700、输入／学习／配置文件强制 0600，并处理已有文件权限，避免只靠 umask。公开 TSV 与产品资源可采用不同权限。测试版缺省保留完整输入本身是已声明的产品选择，但建议改为显式启用并提供保留期限。

普通临时文件采用可预测名称且 `.create(true).truncate(true)` 会跟随已有符号链接，也是加固项；利用需要攻击者能写入目标父目录，不能把同用户任意写文件能力直接算成已验证的提权。

## A7：更新响应限额检查过晚

[fetch.rs:35](../../crates/qingjian-update/src/index/fetch.rs#L35) 先执行 `response.bytes().await` 把完整响应读入内存，然后才检查 2 MiB 上限。索引和 `.sig` 都走此函数；签名验证更晚才发生。因此签名能防止接受伪造索引，**不能防止下载阶段的大响应内存耗尽**。HTTP 超时限制持续时间，不能限制这段时间内累计内存。

条件：更新源被控制、错误配置，或测试覆盖地址返回恶意响应；缺省 HTTPS 阻止普通网络中间人任意伪造，因此没有把这项描述成无需条件的互联网远程攻击。

修复方向：先拒绝明显超限的 Content-Length，并逐块读取、累计超过上限立刻停止；无 Content-Length／chunked 的响应也必须有效。签名文本用独立小限额，云接口响应同样需要应用层最大体积，不能只信 `max_tokens` 请求参数。

## 供应链与恶意行为检查

- 运行时外联主要是用户配置的 AI 服务、签名更新索引；语料工具另调用 Microsoft Translator。这些用途有对应代码与说明。未发现把输入或凭据暗送至另一个硬编码收集端的证据。
- 云服务缺省关闭；Linux 当前启动装配未接入云 Predictor。没有把可选云请求本身定性为恶意外传。
- 自启动、TSF 注册和 uiAccess 的用途与输入法平台实现相符。开发期 `sign-local.ps1` 会安装代码签名证书到本机受信任根／发布者，这是显式开发脚本，不应自动作为普通用户安装步骤执行；本审计没有执行它或任何项目安装／卸载／发布脚本。
- Actions 固定提交、checkout 不保留凭据、Cargo 构建使用 `--locked`；数据包 SHA-256 钉在 `data.lock`；更新索引使用嵌入公钥的 `verify_strict`，只提示不自动安装。上述防护确实存在，但不能代替 IPC 与日志隐私控制。
- 唯一锁定的 Git 来源依赖为 `qingjian-team/cosmic-text` 提交 `9cf0d65...`。已审阅该提交公开补丁：三个文件，字体 optical size 轴处理与缓存变化；未发现新增网络、进程执行或凭据操作。**只核查此提交补丁，未验证整个 fork 历史和全部第三方源码。** [提交](https://github.com/qingjian-team/cosmic-text/commit/9cf0d65a5e4db381e34907103326673386096be3)。
- `.qj` 魔数、种类、映射区边界、UTF-8、词条偏移、CSR 与哈希表已有验证。尚未对全部解析器做覆盖率驱动的 fuzzing；本地模型配置缺少结构合理性和资源上限验证（例如 n_head=0、极大 n_layer/context），需补充恶意模型负面测试，未据此宣称已验证任意代码执行。

初步依赖筛查：下载 RustSec 官方 advisory-db 的 main 压缩快照，SHA-256 为 `42c1e920990b05c2c5ef09ba3b6a8e2b14c69149e7f44a7c63ac504a7f1c6fbf`；筛查 453 个锁定外部包版本，按包名匹配 88 条未撤回公告，再核对 patched／unaffected 范围，未命中已知漏洞或 unsound 版本范围。此阶段用 Python 范围筛查，**不等同于 cargo-audit**，也不覆盖系统动态 OpenSSL、NuGet/MSIX 原生运行时或所有潜在恶意依赖。

两项停止维护告警：

- `paste 1.0.15`：[RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html)，仓库已在 `.cargo/audit.toml` 忽略。
- `ttf-parser 0.25.1`：[RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html)。

停止维护告警不是本项目存在可利用漏洞或恶意代码的证据。

正式 `cargo-audit 0.22.2` 已完成并返回 0：读取锁文件的 473 个包，使用 RustSec 数据库提交 `ef6173cbc5c50ec8166f9a5b28f07834144373ee`（1,290 条公告，最近更新时间 2026-10-03）。没有目标平台过滤，已知漏洞计数为 0；输出 `ttf-parser` 停止维护告警，`paste` 的同类告警按仓库配置忽略。没有修改忽略列表。机器可读原始结果见 [cargo-audit.json](security-audit-2026-10-04/cargo-audit.json)。

## 验证记录与后续

- 已通过：所检查 Bash 安装／卸载／数据获取／发布辅助脚本的 `bash -n` 语法检查。
- Rust：应用户要求在当前 WSL2/Linux 用户环境安装官方 `rustup`，先核对官方公布的 SHA-256，再使用 minimal profile 安装仓库固定的 `1.96.0`，附带 rustfmt、clippy。安装到当前用户的 `/home/chen/.cargo` 与 `/home/chen/.rustup`，未使用 root 安装；已核对 rustc、cargo、rustfmt、clippy 版本，登录 shell 配置会加载 cargo 环境。现有终端可执行 `. "$HOME/.cargo/env"`。这不是 Windows 原生工具链安装。
- rustup 安装器 SHA-256：`dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71`。来源：[Rust 官方安装页](https://rust-lang.org/tools/install/) 与 [rustup 官方安装说明](https://rust-lang.github.io/rustup/installation/index.html)。
- 本机缺少 OpenSSL 开发头文件；未改系统库，使用官方 Ubuntu APT 中与现有运行库同版本的 `libssl-dev 3.0.2-0ubuntu1.30`，核对包 SHA-256 为 `8a47dfa7f9bd4d54ac9dc9a1db14891a9133841bf44f12c3db086f2d4321bac6`，仅解包到 `/tmp/yushi-build-deps`，编译参数显式指向该处及现有系统动态库。本次并未审计系统 OpenSSL 的补丁状态。
- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --exclude qingjian-macos --all-targets --locked -- -D warnings`：通过，无警告。
- `cargo test --workspace --exclude qingjian-macos --locked`：通过，50 个测试结果组共 **641 passed / 0 failed / 1 ignored**；忽略项是 `scorer::latency::batch_latency`。构建产物放在 `/tmp/yushi-security-target`。这是 Linux 环境下的 workspace 测试，Windows 的 `cfg(windows)` 实现没有因此获得平台验证。
- 计数与分组证据：[workspace-test-results.json](security-audit-2026-10-04/workspace-test-results.json)。
- 专项安全验证：**5 passed / 0 failed**。临时将 [probes.rs](security-audit-2026-10-04/probes.rs) 作为 `apps/windows/server/tests/security_audit_20261004.rs`，在 umask 022 下执行 `cargo test -p qingjian-windows-server --test security_audit_20261004 --locked -- --nocapture`；测试后移除该临时文件，没有将确认漏洞行为的断言加入正式测试套件。原始输出见 [probe-results.log](security-audit-2026-10-04/probe-results.log)。探针只使用合成字符串、不调用网络 Predictor、不读取实际输入记录或密钥。
- Windows/macOS 端到端、跨账户 IPC、日志 ACL、真实私密输入控件、实际日志 ZIP，以及覆盖率驱动 fuzzing：尚未运行。
- 未运行项目安装器、卸载器、签名证书安装或发布操作；没有向外部平台发送报告或实际用户输入。生产代码未修复。

建议先修 A1–A4，避免把当前 Windows 版本作为可信的隐私边界；再处理 A5–A7 和负面测试。源码审查、锁文件筛查与测试即使全部通过，也不能证明发布产物与源码完全对应，需要对具体安装包另做校验、签名与行为审计。

### 专项验证所确认的事实

| 探针 | 实际结果 | 证明范围 |
|---|---|---|
| 未 OpenSession 直接发送 Key | Engine 处理按键，组句成为 `n`，会话表仍为空 | Router 不要求会话先注册；不证明 Windows ACL 的实际可达性 |
| 声明已有 SessionId 再 Poll／Commit | 读到 preedit，并取走原始组句 `n` | Router 不知道消息来自哪个连接；管道侧缺少主体传入另由源码确认 |
| 未上报隐私的新会话触发翻译 | 合成选区进入内存 Capture Predictor 的请求 | 确认默认状态与协议链；没有向真实云端发送，没有模拟宿主 InputScope |
| 已上报 Privacy=true 的对照组 | Capture Predictor 收到 0 个请求 | 已知私密状态下 Core 防护有效，问题是平台壳的状态传递时机 |
| umask 022 创建输入日志和配置 | 两个文件均为 0644，配置含合成 API key | Unix 文件模式；跨账户读取还依赖祖先目录权限 |

探针成功意味着重现了所述现有行为，**不意味着这些安全问题已经修复**。
