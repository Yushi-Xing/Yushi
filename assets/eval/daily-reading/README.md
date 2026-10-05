# 日常文章输入质量固定集

四篇已实际检索的公开中文文章，来源与发布日期见 [sources.json](sources.json)。每篇选 8 个常用词、1 段极短原文和 7 条原创日常改写，合计 32 词、32 句；不是全文复制，也不是完整新闻事实准确性评测。原文短段分别为“我打开笔记本电脑”“文具放在笔筒里”“站内导向标识清晰”“倡导科学健康生活理念”。其余句子只是围绕文章主题编写的输入样例。

[seeds.json](seeds.json) 人工给出无声调拼音，每个汉字对齐一个音节。冻结后不得为提高分数修改标准答案。每篇 4 条为独立留出项，合计 16 条；其余 48 条用于开发分析。留出集与开发集源于相同文章，规模很小，不能代表独立人群或整个中文输入分布。

原始种子生成 1,123 条唯一组合（其中 4 条实际用户输入另列）：正常全拼、首／中／尾漏键、多键、邻键错误、远距离元音误键、跨音节相邻交换、双漏键、双邻键错误、漏键加多键、全简拼和交错简拼。重复变体去重，各组分母可能不同。用户单引号是界面自动分隔，测试使用实际按键串。

```bash
python3 tools/eval/daily_reading.py --binary target/release/qingjian-cli --output /tmp/daily-static
python3 tools/eval/daily_reading.py --binary target/release/qingjian-cli --output /tmp/daily-model --model data/models/hanzhang-tongbian/hanzhang-tongbian-small.qjm
python3 tools/eval/test_daily_reading.py
```

不读用户配置，不学习，不开启云服务，不将目标文字加入词库。每次记录二进制、种子、来源、主词库、补丁与模型哈希，并核对候选排名和实际字符编辑距离；解析失败保留在分母。`--probe` 只运行 100 条正常／用户／句子双漏键子集，输出显式标记，不能冒充完整集。

输出完整候选与 `failures.jsonl`，分开统计词／句、文章来源、开发／留出及实际用户用例。仅有纠正标记不算成功。当前集包含“的／地、哪／那、再／在”等上下文歧义；这些是预先固定的期望表述，不是断言其他语法形式在任何上下文里都错误。
