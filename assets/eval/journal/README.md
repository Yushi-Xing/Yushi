# 中文期刊主题固定评测集

查阅以下期刊官网摘要后，按其研究主题自行改写测试句子；这些句子不是论文原文摘录，也不是论文的实验数据。人工指定逐字无声调读音，固定为全拼，避免用待测引擎为自己生成标准答案。未把测试句子加入产品词库，未训练模型。

| ID | 文章与来源 | 发布信息 |
|---|---|---|
| agents | [基于大语言模型智能体的代码生成综述](https://rjxb.alljournals.cn/jos/article/abstract/7593) | 软件学报，2026，37(8):3089–3115；DOI 10.13328/j.cnki.jos.007593 |
| testing | [面向智能软件系统的测试用例生成方法综述](https://rjxb.alljournals.cn/jos/article/abstract/7450) | 软件学报，2026，37(1):62–101；DOI 10.13328/j.cnki.jos.007450 |
| forest | [湖南省森林植被碳储量、碳密度动态特征](https://www.ecologica.cn/html/2016/21/stxb201504230837.htm) | 生态学报，2016，36(21):6897–6908；DOI 10.5846/stxb201504230837 |
| health | [社区赋能在慢性病防控中的应用研究](https://www.chinagp.net/CN/10.12114/j.issn.1007-9572.2019.10.005) | 中国全科医学，2019；DOI 10.12114/j.issn.1007-9572.2019.10.005 |

`seeds.json` 有 24 条期刊主题原创句和 6 条日常对照句。`tools/eval/journal.py` 确定性构造 336 个输入：30 条原拼音、30 条带上文、240 条单处错拼（中间/末尾的换位、漏键、重键、替换）、30 条双处错拼、6 条显式模糊音。不同拼音、上文都单独计数，不删除失败样本。单处错拼可能产生另一种合法拼音，不能假定引擎应始终猜回原句。

```bash
cargo build --release -p qingjian-cli --locked
python3 tools/eval/journal.py --binary target/release/qingjian-cli --output target/journal-eval
# 同时测随包 P2C 模型：
python3 tools/eval/journal.py --binary target/release/qingjian-cli --output target/journal-eval-model --model data/models/hanzhang-tongbian/hanzhang-tongbian-small.qjm
```

首选/前三/前五以全部输入为分母，解析失败计错。字错误率 CER = 实际首选与标准答案的 Unicode 编辑距离之和 / 标准答案字符总数；插入、删除、替换均计错。纠正标记数量不等于纠错成功数量，成功必须匹配标准答案；音节级纠错未必产生整串纠正标记。保留完整失败明细、数据/模型/种子 SHA256，便于复核。

这是规模较小的固定诊断集，且变体彼此相关，不能代表全部中文输入准确率；医学主题只是文字输入测试。宿主输入、光标、焦点和上屏问题由 Windows 自动化和人工矩阵另行验收。
