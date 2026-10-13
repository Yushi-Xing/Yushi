# Windows 第一批基础修复

本轮从 `1ef14cb37dad2ffa4e5c1264468c7d831292e7cc` 开始，按 [下一轮计划](../plan/windows-next-2026-10-13.md) 引入五项独立修复，预览版本为 0.1.6-beta.13。未合并整个上游 main，未改 data-v3、19,662 条补库、双拼数量或 SearchHost 托管协议。

## 代码与来源

| 改动 | 移植来源 | 本地适配及回归 |
| --- | --- | --- |
| Windows 自定义短语装配 | [ed0cae72ae](https://github.com/qingjian-team/qingjian/commit/ed0cae72ae6ca31904d4b31cf1172529bf4101f4) | 启动与热加载共用 helper；增删、替换、停用、重新启用、非法位置保持旧值、多行及 emoji 上屏；警告不含短语正文。 |
| j/q/x/y 后 v 全拼别名 | [05767e9d99](https://github.com/qingjian-team/qingjian/commit/05767e9d99dda7af39784c565c6a215a59084d23) | 合法音节、词库规范化与双拼解码一起适配，保留原按键显示；句子与七套双拼往返回归。 |
| 模型字表外候选回退 | [6369ba1c3f](https://github.com/qingjian-team/qingjian/commit/6369ba1c3fbda5fa44adcb820a3545f66975f8c3) | 两个 SentenceScorer 遇任一未知字符整批跳过；合成 CPU 模型验证未知字符在首、中、末位置、罕见字、emoji、空批次和后续已知批次。 |
| 隐藏词库文件过滤 | [be0383c9f2](https://github.com/qingjian-team/qingjian/commit/be0383c9f2fb8340a6834708439c1b48350c5c43) | 点开头文件不列出、不加载、不触发快照变化；保留同名 qj 优先、Unicode 文件名。 |
| 损坏的预发布版本 | [2368e5774d](https://github.com/qingjian-team/qingjian/commit/2368e5774d1979619b9a261b4069186d3fa0fe0f) | 拒绝空点分段与孤立尾横线，保留现有开发版和数字比较行为。 |

本地另补模型拒绝批次的缓存：拒绝或评分长度不匹配时，本输入代、同一读法不再尝试；输入/前文变化后可重新尝试。新测试先复现同步 20 次重复打分与异步拒绝结果不通知，再修复并验证静态排序、异步收敛与新读法。该保护防止拒绝回退导致重复请求，不等于用户笔记本功耗或电流声已验收。

初次 `66a5eae` 提交准备 beta.12 时，发现 CLI 的严格实验 P2C 适配器直接调用原始评分，没有跟随产品适配器跳过字表外候选。已取消 beta.12 发布运行 `38411478996`，该版本不提供安装器；保留标签作记录，不移动已推送标签。补齐评测规则并新增随包模型正负测试后，重新对照与发布 beta.13。严格实验仍在真正推理错误时终止，不能把坏模型的静默回退冒充准确度成功。

## 验证范围

本机运行 Linux 可移植工作区（排除 macOS 壳）、fmt、Clippy，以及六个 Python 评测/词库/发布门禁脚本。真实 Windows MSVC x64、x86 DLL、模型停键探针和完整冻结评测由 Windows preview Actions 执行；真实 Win10/Win11 应用验收继续待用户安装。

本机工作区、Clippy、fmt 与 Python 的最终结果在证据汇总中；随包模型的 CLI 回退用例额外显式运行。Windows 发布门禁单独运行真实模型停键与原生诊断，不用忽略项冒充验收。

补齐评测后的最终结果：工作区 713 通过、0 失败、4 忽略；另显式运行随包模型 CLI 回退测试 1 项通过，Python 14 项通过。100 条正常/双漏键/用户例对照没有首选变化或旧成功项回退；正常首选无模型 43/64、有模型 42/64，正常整句均为 17/32，双漏键均为 0/32，尚未达到整句改善目标。

失败基线先证实别名、短语、未知字符、隐藏文件及版本校验的断言失败。沙箱禁止 TCP 回环导致的七个旧 HTTP 测试失败属于运行权限限制；允许本机回环后全通过，未因此修改网络实现。

本轮未安装新 Rust 或替换系统库。OpenSSL 开发包由 Ubuntu 官方索引下载并验证 SHA256 后仅解包到 `/tmp/yushi-build-deps`；构建目录 `/tmp/yushi-next-target`，限制两个构建线程。测试模型为虚构数据；文章探针只使用仓库冻结材料，关学习、关网络。

首次构建产生约 14 GiB 调试缓存，已清理；补齐评测入口后使用 `/tmp/yushi-cli-target`，关闭调试符号与增量缓存，继续限制两个构建线程。

文章探针对照包含 64 条正常词句、32 条双漏键和 4 条用户回归，共 100 条；词库、模型、语料哈希和前后结果保存在 [证据目录](windows-foundation-eval/README.md)。模型对照是同一冻结输入的 P2C 重排，生成另由真实停键探针覆盖，不混成完整产品生成准确率。调试构建耗时只用于诊断，不作为发布性能结论。Windows 完整评测随发布附件提供，不把旧 beta.11 的附件结果当成本轮实测。

## 仍未完成

第二批 TSF 吞键/数字/定位/UIAccess 修复、IPC 身份与资源限额、整句多处纠错和排序、数据-v7 增量、安装包分组测量、轻量设置及按需英语句译均未由本批实现。SearchHost 路径没有改动，仍需实际搜索栏验收；整句首选目标仍需单独优化。

## 发布与交叉核验

[beta.13 Release](https://github.com/Yushi-Xing/Yushi/releases/tag/yushi-windows-v0.1.6-beta.13) 已由 [Windows 发布门禁](https://github.com/Yushi-Xing/Yushi/actions/runs/38413715436) 完成，来源 `e4ed934c5145998d7ad03799a0bc9ed5884a5d0e`。本机下载全部六项附件，核对 GitHub digest、SHA256SUMS、安装器 MZ 文件头、三个 ZIP 的 CRC 和 CI 汇总/提交一致性；[核验记录](windows-foundation-release-proof.json) 保存完整指标。安装器 140,036,116 字节，约 133.55 MiB，SHA256 `7872f5a8662d18867c72c526e2c71735e0b3b53d53092b0c1a9872a0c8dc399c`。

Windows 默认工作区 732 通过、0 失败、4 忽略；原生诊断、真实模型停键、CLI 真实模型规则各显式运行 1 项通过，总计 735 通过。剩余未显式执行的忽略项是神经批量延迟探针。停键四例全部零新增调用；CI CPU 样本不是用户笔记本异响验证。

完整冻结集为期刊 306 条、拼音组合 1,444 条、日常文章静态/模型各 1,123 条、补库能力 237 条。另做本机相同 1,444 条前后对照，正常 84/92、单处错拼 757/1104 保持；一个原失败样本首选变化，旧成功项零回退，见 [组合对照](windows-foundation-eval/quality-comparison.json)。后续整句定位见 [诊断](windows-sentence-diagnosis-2026-10-13.md)。

同一来源提交的常规 CI 中，Linux/macOS 通过，但 Windows 旧热加载测试在末尾删临时目录时返回 Win32 145（DirectoryNotEmpty）；发布门禁的完整 Windows 运行通过。代码检查发现这两项旧测试以缺省配置启动了真实更新 Checker，后台落盘可能与清理竞争。后续只修改测试夹具：所有有效配置显式 `check = false`，每次轮询前后断言不启动更新，保留业务和目录清理断言；不把删目录错误吞掉或靠重试掩盖。常规 Windows CI 额外连续运行热加载测试 10 次，每次必须成功，失败立即拒绝。该改动不改变 beta.13 产品行为，后续常规 CI 验证另记。

两轮本机临时编译缓存（约 14 GiB 与 2 GiB）及冗余调试 CLI 已清理，仅保留对照二进制、报告及发布核验下载；未对 WSL 虚拟磁盘执行压缩或系统清理。

夹具隔离提交 `87399d20aca85be312999e5e9cc6c3b42c458a43` 的 [常规 CI](https://github.com/Yushi-Xing/Yushi/actions/runs/38420602191) 三个平台全部通过；Windows 热加载四项测试连续 10 轮全部通过。它只改测试、CI 与证据，beta.13 安装器仍对应 `e4ed934`，没有移动标签或替换已发布附件。
