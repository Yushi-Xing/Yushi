# Yushi-Xing/Yushi 与青简上游的改动分析

## 对照范围与结论

用户所指的仓库为 [Yushi-Xing/Yushi](https://github.com/Yushi-Xing/Yushi)，它是 [qingjian-team/qingjian](https://github.com/qingjian-team/qingjian) 的 fork。本次通过 GitHub 元数据、远程 main 指纹、Git 历史及实际源码差异核对；没有仅凭旧计划中的勾选状态判断功能已完成。

| 对照项 | 固定提交 |
| --- | --- |
| Yushi-Xing main | `f5564e4df8c4af9cb58652f4e3853eea53dae8fe` |
| 与查询时青简 main 的最近公共祖先 | `b7a4f8bbb124c7e4df7660f6c2ce97b627113205` |
| 查询时青简 main | `7e76aba5425532df476f702f5a26b3baada87567` |
| 我们已验证的 Windows 安装包源码 | `7d2fb9d054238827f5d4c03bdd7fcfc43c1ab5b1` |

Yushi-Xing main 在该公共祖先之外有 25 个提交，其中 23 个非合并提交、2 个上游合并提交。相对公共祖先的树差异为 250 个文件、72,740 行增加、384 行删除；其中大量是词表、逐词来源、评测样例及历史证据，不等于新增了七万行引擎逻辑。不能直接拿最新青简与较旧 Yushi 的全部树差异当作 Yushi 自己实现的功能：其中还包含青简后续新增、Yushi 尚未同步的代码。

我们最初的 `origin/main` 就是 `f5564e4d`，且它是当前源码的祖先。因此下述改进已包含在当前工作中，不需要再 cherry-pick 一次。固定上游仍为青简；Yushi-Xing 是本次研究对象。最新查询的青简 main 比本轮已同步的 `94ed1917` 又多 5 个可达提交，本次分析没有合并这些更新，避免改变已通过原生构建验收的产品代码。

## 实际改动

| 范围 | Yushi 的改动与作用 | 源码与主要提交 |
| --- | --- | --- |
| 会话隔离 | 将外部会话编号限制在连接内，映射到 Server 的唯一编号；未登记请求拒绝，每连接最多 64 个会话，断线清理；后台会话的 Commit 不再清空前台输入 | `apps/windows/server/src/ipc/session_map/`、`dispatch/session/`；`241238e1` |
| 日志与诊断 | 去掉 TSF 逐键字符、拼音、上屏正文日志；诊断配置只导出允许字段，省略密钥、地址、自定义短语等；随机暂存目录、日志数量/字节限额和已知密钥掩码；PowerShell 路径保留 Unicode/特殊字符 | `crates/qingjian-platform/src/diagnostics/`、`tests/native_diagnostics.rs`；`241238e1`、`df9d812b` |
| 更新读取 | 对索引和签名分别设 2 MiB/1 KiB 上限，无 Content-Length 或 chunked 响应也累计限制；保留验签 | `crates/qingjian-update/src/index/fetch/`；`241238e1` |
| Windows 候选协商 | 通过 `ITfUIElementMgr`、候选列表行为接口和搜索候选 provider 与宿主协商显示；宿主选择自己显示时通知 Server 隐藏独立候选窗，支持选词/翻页；搜索宿主允许自绘时增加宿主进程内子窗口回退 | `apps/windows/tsf/src/com/candidates/`、`service/candidates.rs`；`9c2d3723`、`4cca0d5a` |
| 模型资源 | 同一输入按读法缓存，避免多个读法互相清缓存；批量提交各读法，输入换代丢弃旧结果；最多 1,024 个分数槽位；拒绝或未知字表批次不再不断重试；停键探针断言不新增模型调用 | `crates/qingjian-core/src/engine/rescoring/`、`apps/windows/server/tests/model_idle.rs`；`d271b2fe`、`66a5eaef` |
| 拼音与纠错 | 显式模糊音优先于泛化纠错，补“嗯”等读音；用实际输入测试全拼/简拼混合、尾字简拼；完整中文纠错首选不会轻易被模型生成的不同长度内容抢走 | `crates/qingjian-core/src/fuzzy/`、`engine/parsing.rs`、`engine/query/converting.rs`；`241238e1`、`9c2d3723`、`526bd4c1` |
| 常用词补充 | 从有许可的结巴、THUOCL 与 Unihan 筛选补充词，保留逐词来源；领域原始 DF 与人工排序权重分开，不把排序参数宣称为实测词频 | `assets/lexicon/supplement/`、`tools/lexicon/`；`daa9ff57`、`11e2d683` |
| 上游适配 | 移植 Windows 短语装配/热加载、j/q/x/y 后 v 别名、隐藏词库过滤、坏预发布版本校验、模型字表外回退；后续合并上游保留原始按键，使用拼音规范显示 ü | `66a5eaef`、`e4ed934c`、`a953125c`、`7fc57b86`；这些并非全由 fork 首创 |
| 翻译与协议兼容 | 对相同候选保留译文缓存，进入私密状态时清缓存并取消请求；补原始按键字段与按旧 DLL 能力降级，降低升级后未重启应用的协议不兼容风险 | `apps/windows/server/src/dispatch/composed/`、`dispatch/session/`、`crates/qingjian-platform/src/protocol/` |
| 评测与发行 | 修正按文本去重丢错拼、排除解析失败、借答案长度挑候选等统计偏差；增加 1,444 条拼音组合、1,123 条日常文章、336 条期刊题材及 237 条词库抽测；固定 LF，校验数据/模型/二进制哈希；独立预发布门禁保留 x64/x86 TSF 和证据附件 | `tools/eval/`、`.gitattributes`、`.github/workflows/windows-preview.yml`；多轮 beta 提交 |

词表数量以当前固定树中的 manifest 为准：19,469 条、499,077 字节。早期 beta.10 记录的 19,662 条是当时版本，不能当作当前计数。Yushi 的锁定产品数据为 data-v8，我们本轮同步后使用 data-v9。

## 效果与尚未解决的问题

这是保留跨平台架构、重点加强 Windows 的 fork，尚未完成“只保留 Windows、替换成轻量原生设置、独立按需句译”等早期规划。公共模型和补库也不能保证整句准确率同步提高：其历史文章评测显示补库改善词形覆盖，但正常文章首选没有随之增长，见 [补库评测](windows-refactor-round4-eval/README.md)。

会话映射提供连接内隔离，不等于管道账户身份认证。广泛管道 ACL、客户端/服务端身份校验、连接与队列额度、读写截止时间仍需专门处理，不能把日志脱敏描述为完整安全加固。未知历史密钥和旧日志也不能保证自动清除，见 [安全修复范围](windows-refactor-round1-2026-10-04.md)。

搜索宿主内候选实现和原生窗口测试存在，仍不足以证明 Win11 搜索/开始菜单遮挡解决；未签名版本的 UIAccess 条件也不能靠安装目录替代。用户真实场景继续待验收，见 [当前遮挡排查](windows-occlusion-2026-11-05.md)。

## 我们在此基础上新增的工作

本轮 `548ba087` 同步青简上游后，保留上述 fork 改动；`dcd97cff` 再修整句评分漏用已上屏上下文、重复词挤占有限候选名额，并减少无效搜索/分配；选词后台任务只保留最新待办，同一请求去重，清空/上屏/换会话后取消过期工作。release 开启 ThinLTO 与符号剥离，标准 Windows 包使用共享设置，旧 WinUI 及运行库改为兼容构建选项。性能实测和边界见 [优化报告](input-efficiency-2026-11-05.md)。

`e600212f` 新增 80 条冻结姓名/语句种子和 290 条按键抽测，及原生 Windows 构建/安装/启动/渲染/卸载门禁；`7d2fb9d0` 修正 Inno 将字符串 `"0"` 当真导致误带旧设置的问题，增加真实 Inno 编译回归。

原生 Windows 验证：[成功运行 41977249531](https://github.com/likeplayscav/Yushi/actions/runs/41977249531)。默认 workspace 1,750 项通过、0 失败，显式补跑诊断导出、实际模型停键、模型字表外回退 3 项均通过；仍有旧模型延迟探针、联网模型下载、忽略的文档示例未执行，不记为通过。安装、两种架构 COM 注册、三个程序启动、33 张设置离屏渲染及卸载检查通过。交互式 Win11 搜索未验证。

全拼抽测默认首选/前五：常见姓名 8/24、21/24；公共人物名 13/16、16/16；日常短句 32/32、32/32；含姓名句子 4/8、5/8。开启通变后常见姓名首选仅升至 9/24，姓名排序仍是弱项。样例较小，不能外推全部中文准确率。

已验证安装器 `qingjian-0.3.3-dev-7d2fb9d-windows-x86_64-setup.exe` 为 75,976,019 字节（72.46 MiB），SHA256 为 `9c66f2913673a022b9b33347f89317399f829874279f5a32620ce8b8b4031821`；[下载与验证附件](https://github.com/likeplayscav/Yushi/actions/runs/41977249531/artifacts/13393454313)。这对应源码 `7d2fb9d0`，后续只加文档不会改变该二进制来源。安装引导程序为 PE32，安装的主程序已验证为 x86_64，并另带 x86 TSF。

## 可复查的 Git 命令

```bash
git merge-base upstream/main yushi-xing/main
git log --no-merges --oneline upstream/main..yushi-xing/main
git diff --stat b7a4f8bbb124c7e4df7660f6c2ce97b627113205 f5564e4df8c4af9cb58652f4e3853eea53dae8fe
git merge-base --is-ancestor f5564e4df8c4af9cb58652f4e3853eea53dae8fe HEAD
```
