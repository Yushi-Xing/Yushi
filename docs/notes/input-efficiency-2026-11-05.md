# 输入准确度、资源与包体优化结果

本轮在上游同步后的 `b0b52093` 上实施，分支 `perf/input-efficiency`。方案见 [实施顺序](../plan/input-efficiency-2026-11-05.md)，原始数值与指纹在 [metrics.json](input-efficiency-2026-11-05/metrics.json)，逐条准确度对照在 [accuracy.jsonl](input-efficiency-2026-11-05/accuracy.jsonl)。

## 分析与落地

| 范围 | 发现 | 本轮处理 |
| --- | --- | --- |
| 拼音解析、词库、整句搜索 | 只有一种同级切分也会先求一次整句用于比较；单前驱搜索为每个词分配临时 Vec | 省去无效比较与临时分配，保留原搜索规模 |
| 多词库与候选准确度 | 相同词的词频不同，按得分排序后不相邻；相邻去重会漏掉重复，挤掉其他词 | 全局按文字去重后再截取格子名额；新增不同词频的重叠词库回归 |
| 连续输入与学习上下文 | 词级排序使用已上屏词，整句却始终从 `Context::START` 开始 | 通过 `SearchInput::context` 传递上屏链，起始词与第二个词都保留正确的二元/三元上下文；诊断和拼接路径同步 |
| 本地模型后台任务 | 知意的通道可堆积任务，输入改变或卸载后仍可能算剩余读法；上屏/清空后待办还在 | 单槽待办合并通知；输入条件与任务序号取消过期工作；清空、上屏、原样输出、会话切换同时取消知意与含章的剩余任务 |
| 文件与常驻内存 | 已采用 mmap；本次加载的主词库约 15 MiB、通用词库约 66 MiB、语言模型约 43 MiB | 保持相同数据与覆盖范围，实际 RSS 基本持平；不把代码体积下降等同于数据内存下降 |
| 渲染、系统壳与 IPC | 候选帧已去重，Windows DLL 定时器仍按 80 ms 唤醒；这与模型线程是否空闲是两件事 | 检查共享 Core 改动的 Windows 类型兼容；真实 UI 的唤醒、功耗与遮挡仍需要 Win11 测量，不能由 CLI 数据推出 |
| Windows 安装包 | 新共享设置与旧 WinUI 设置、完整 Windows App Runtime 同时打包 | 标准包只带共享设置；`-IncludeLegacySettings` 显式生成兼容包；已安装旧版时菜单仍提供入口 |
| release 二进制 | 跨 crate 优化和符号剥离未配置 | 开启 ThinLTO、单 codegen unit、符号表剥离；保留 panic 展开和平台回调隔离 |

没有修改词库、语言模型权重或评测答案。云联想和本地模型的默认开关保持当前产品设置，主要性能指标来自无模型的日常输入路径。

## 准确度

所有对照使用相同的 `data-v9`、配置和冻结输入。基线与优化后二进制的 SHA256 分别归档；模型探针也记录相同模型文件的 SHA256。

| 评测 | 基线 | 优化后 |
| --- | ---: | ---: |
| 输入质量集：正常输入 Top-1 | 83 / 92 | 83 / 92 |
| 输入质量集：错拼 Top-1 | 687 / 1,104 | 687 / 1,104 |
| 日常文章集：正常输入 Top-1 | 47 / 64 | 47 / 64 |
| 日常文章独立留出组：正常输入 Top-1 | 10 / 16 | 10 / 16 |
| 通变模型探针：正常输入 Top-1 | 47 / 64 | 47 / 64 |

完整无模型集合共 2,804 条（1,444 + 1,123 + 237），模型探针另有 100 条。逐条比较中，原本 Top-1、Top-5 成功的输入均未丢失，也没有新增成功项；一条原本失败的重错拼输出发生变化，仍为失败。补库抽测是词库能力检查，不与独立文章准确率混算；100 条探针不是完整模型评测。

上下文缺陷用受控模型单独复现：先上屏「需要」，再输入 `banlishouxu`，旧代码即使得到支持「需要 → 办理」的模型分，仍输出「班里手续」；修复后输出「办理手续」，断链后恢复句首首选。测试先失败后通过。它证明上下文接线已修复，不代表上述无前文冻结集的总体命中率已经提高。同音歧义和语言模型评分仍是后续准确度的主要空间。

## 逐键延迟与资源

Linux x86_64 云 VM，Rust 1.96.0 release；固定 15 组输入、每个版本三轮，共 780 个按键样本。包含普通长句、中英混输、错误与重复声母。逐键原始数据：[基线](input-efficiency-2026-11-05/before-typing.jsonl)、[优化后](input-efficiency-2026-11-05/after-typing.jsonl)。

| 指标 | 基线 | 优化后 | 变化 |
| --- | ---: | ---: | ---: |
| 每键 P50 | 1.58 ms | 1.33 ms | -15.8% |
| 每键 P95 | 6.10 ms | 4.60 ms | -24.6% |
| 每键平均 | 2.183 ms | 1.777 ms | -18.6% |
| 最慢一键 | 12.32 ms | 8.54 ms | -30.7% |
| 每轮进程 CPU 时间平均 | 0.780 s | 0.657 s | -15.8% |
| 峰值 RSS（三轮最大） | 173.9 MiB | 173.6 MiB | 基本持平 |
| Linux CLI 文件大小 | 19,163,696 B | 13,825,912 B | -27.9% |

这些是同机观测值，VM 调度仍有波动。CLI 标注整份候选，记录的是解析、查词、排序、标注阶段之和，包含启动的进程 CPU / RSS 单独列出；不包含 Windows 窗口绘制、TSF 或完整端到端按键耗时。本次固定样本均低于每键 10 ms，不能据此保证所有设备和输入都达标。

后台模型的阻塞测试覆盖：卸载后剩余批次停止、100 个连续新请求只保留最新待办、取消后丢弃已入队结果，以及清空/上屏后不再提交旧请求。同一输入连续查询 100 次也只保留原来的在途任务序号。单次已开始的模型前向仍不可强行中断，所以取消不等于瞬间零 CPU；此处没有测量用户笔记本的功耗或噪声。

## 安装包

当前公开 0.3.2 Windows 安装包为 88,888,970 字节，SHA256 已核对。但这与含剪贴板等更新的当前源码不是同一版本，因此不能据此计算本轮压缩包缩小多少。现有提取工具也不支持该 Inno 版本，未把文档中的旧运行库体积当成新测量。

本轮实际交付了标准/兼容两条打包路径和体积清单：每次原生打包产生 `package-size.json`，逐文件记录来源、字节数和分组，另记安装包字节数及 SHA256。标准包明确不包含旧设置及其运行库；兼容包仍要求完整运行库。两个包均保留 x64/x86 TSF、原词库、所有学习语言和 ONNX 支持。升级时不主动清除既有兼容文件，因此已有安装的磁盘占用不一定随新下载包一起下降。

使用真实 PowerShell 运行打包入口、隔离的文件制品和 ISCC 替身，已验证：标准包无需旧设置即可通过；兼容包缺运行库拒绝、齐全后收集并传正确开关；缺 32 位 DLL 或学习语言数据均拒绝。它验证构建逻辑和清单，不替代 Inno 编译、Windows 安装卸载和 GUI 验收。

## 复现

```bash
tools/release/data-fetch.sh
cargo build --release --locked -p qingjian-cli
python3 tools/eval/input_quality.py --binary target/release/qingjian-cli --output target/check-quality
python3 tools/eval/daily_reading.py --binary target/release/qingjian-cli --output target/check-daily
python3 tools/eval/lexicon.py --binary target/release/qingjian-cli --output target/check-lexicon
python3 tools/eval/typing.py --binary target/release/qingjian-cli --output target/check-typing
python3 tools/release/test_windows_installer.py
```

输出目录需要尚不存在。PowerShell 测试自动查找 `pwsh` / `powershell`，也可通过 `QINGJIAN_TEST_POWERSHELL` 指定路径；Windows CI 已接入该门禁。详细验证结果归档于 `metrics.json` 的 `checks`。

最终源码通过 `cargo fmt --all --check`、Linux workspace 严格 Clippy 与 1,733 项 Rust 测试（0 失败，6 忽略；Linux 排除 macOS 壳），Windows GNU 目标的 Server / TSF 全 targets 严格 Clippy，以及 9 项 Python 打包与评测工具测试。

Windows 原生安装包与 Win11 交互尚未在本环境构建/验证；此前的搜索遮挡问题继续见 [独立排查记录](windows-occlusion-2026-11-05.md)。
