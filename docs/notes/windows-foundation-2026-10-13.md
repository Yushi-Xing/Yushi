# Windows 第一批基础修复

本轮从 `1ef14cb37dad2ffa4e5c1264468c7d831292e7cc` 开始，按 [下一轮计划](../plan/windows-next-2026-10-13.md) 引入五项独立修复，预览版本为 0.1.6-beta.12。未合并整个上游 main，未改 data-v3、19,662 条补库、双拼数量或 SearchHost 托管协议。

## 代码与来源

| 改动 | 移植来源 | 本地适配及回归 |
| --- | --- | --- |
| Windows 自定义短语装配 | [ed0cae72ae](https://github.com/qingjian-team/qingjian/commit/ed0cae72ae6ca31904d4b31cf1172529bf4101f4) | 启动与热加载共用 helper；增删、替换、停用、重新启用、非法位置保持旧值、多行及 emoji 上屏；警告不含短语正文。 |
| j/q/x/y 后 v 全拼别名 | [05767e9d99](https://github.com/qingjian-team/qingjian/commit/05767e9d99dda7af39784c565c6a215a59084d23) | 合法音节、词库规范化与双拼解码一起适配，保留原按键显示；句子与七套双拼往返回归。 |
| 模型字表外候选回退 | [6369ba1c3f](https://github.com/qingjian-team/qingjian/commit/6369ba1c3fbda5fa44adcb820a3545f66975f8c3) | 两个 SentenceScorer 遇任一未知字符整批跳过；合成 CPU 模型验证未知字符在首、中、末位置、罕见字、emoji、空批次和后续已知批次。 |
| 隐藏词库文件过滤 | [be0383c9f2](https://github.com/qingjian-team/qingjian/commit/be0383c9f2fb8340a6834708439c1b48350c5c43) | 点开头文件不列出、不加载、不触发快照变化；保留同名 qj 优先、Unicode 文件名。 |
| 损坏的预发布版本 | [2368e5774d](https://github.com/qingjian-team/qingjian/commit/2368e5774d1979619b9a261b4069186d3fa0fe0f) | 拒绝空点分段与孤立尾横线，保留现有开发版和数字比较行为。 |

本地另补模型拒绝批次的缓存：拒绝或评分长度不匹配时，本输入代、同一读法不再尝试；输入/前文变化后可重新尝试。新测试先复现同步 20 次重复打分与异步拒绝结果不通知，再修复并验证静态排序、异步收敛与新读法。该保护防止拒绝回退导致重复请求，不等于用户笔记本功耗或电流声已验收。

## 验证范围

本机运行 Linux 可移植工作区（排除 macOS 壳）、fmt、Clippy，以及六个 Python 评测/词库/发布门禁脚本。真实 Windows MSVC x64、x86 DLL、模型停键探针和完整冻结评测由 Windows preview Actions 执行；真实 Win10/Win11 应用验收继续待用户安装。

最终本机工作区 713 通过、0 失败、3 个需外部数据或平台的显式测试忽略；Clippy `-D warnings` 与 fmt 通过，Python 共 14 个测试通过。Windows 发布门禁会单独运行真实模型停键与原生诊断，不用这些忽略项冒充验收。

失败基线先证实别名、短语、未知字符、隐藏文件及版本校验的断言失败。沙箱禁止 TCP 回环导致的七个旧 HTTP 测试失败属于运行权限限制；允许本机回环后全通过，未因此修改网络实现。

本轮未安装新 Rust 或替换系统库。OpenSSL 开发包由 Ubuntu 官方索引下载并验证 SHA256 后仅解包到 `/tmp/yushi-build-deps`；构建目录 `/tmp/yushi-next-target`，限制两个构建线程。测试模型为虚构数据；文章探针只使用仓库冻结材料，关学习、关网络。

文章探针对照包含 64 条正常词句、32 条双漏键和 4 条用户回归，共 100 条；词库、模型、语料哈希和前后结果保存在 [证据目录](windows-foundation-eval/README.md)。调试构建耗时只用于诊断，不作为发布性能结论。Windows 完整评测随发布附件提供，不把旧 beta.11 的附件结果当成本轮实测。

## 仍未完成

第二批 TSF 吞键/数字/定位/UIAccess 修复、IPC 身份与资源限额、整句多处纠错和排序、数据-v7 增量、安装包分组测量、轻量设置及按需英语句译均未由本批实现。SearchHost 路径没有改动，仍需实际搜索栏验收；整句首选目标仍需单独优化。
