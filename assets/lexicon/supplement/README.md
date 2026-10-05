# 常用补充词库

Windows 与 CLI 默认加载这份补充库，不需要打开本地模型、云服务或领域词库。基础 `.qj` 产品数据保持原锁定版本，补充库编入程序并在词库热加载时保留。

当前共 **19,662** 个词形，文本约 **492 KiB**；与 `assets/lexicon/dict.tsv` 和原人工补丁按词形去重。包含日常组合、饮食、财经、计算机、汽车及部分地名相关词，以及人工核验的品牌和生活用品，例如双汇火腿肠、康师傅方便面、充电宝、抽纸、付款码。

## 来源与筛选

- [结巴词表](https://github.com/fxsjy/jieba/blob/67fa2e36e72f69d9134b8a1037b83fbb070b9775/jieba/dict.txt)：固定提交 `67fa2e36e72f69d9134b8a1037b83fbb070b9775`，MIT，保留 [许可证](JIEBA-LICENSE.txt)。源频次不少于 20，只收 2—8 个规范汉字组成的词；人名、地名须已有可核验的整词读音才收。
- [THUOCL](https://github.com/thunlp/THUOCL)：复用仓库已有原始分类表和整词读音，MIT，保留[许可证](../00_meta/THUOCL_LICENSE.txt)。饮食、财经、计算机、汽车按各自 DF ≥ 1000 筛选；地名按 DF ≥ 10000 筛选。各领域 DF 仅用于选词，不把不同语料的 DF 合并为词频。
- [Unihan 17.0.0](https://www.unicode.org/Public/17.0.0/ucd/Unihan.zip)：取 `kMandarin`、`kHanyuPinlu`、`kXHC1983` 的所有普通话读音，Unicode License V3，保留[许可证](UNICODE-LICENSE.txt)。无整词读音的词，只有每个字均为单音字才自动标音；含多音字的未知词暂不导入。
- [reviewed.tsv](reviewed.tsv)：项目人工核验的读音与保守排序权重；品牌组合由常见品牌与商品组成，通用词为日常生活用语。读音仍逐字对照 Unihan 检查。按本仓库许可证分发。

自动补词的第三列为 20—500 的保守排序权重，**不是统一语料下的实际词频**；人工核验词的权重同样是排序参数。补入完整词条不能保证所有同音词都成为首选，尤其是整句转换仍依赖语言模型与路径排序。

`provenance.jsonl` 逐条保存来源、原始频次或 DF、读音来源与排序权重；`manifest.json` 保存数量、筛选条件、输入和输出 SHA256。未知读音、多音字冲突及未核验专名分别计入拒绝统计。未收录词不会因读音猜测而强行入库。

## 离线复现

从上述固定 URL 下载结巴 `dict.txt`，从 Unicode 17.0.0 的归档提取 `Unihan_Readings.txt`，然后运行：

```bash
python3 tools/lexicon/supplement.py --jieba /path/to/dict.txt --unihan /path/to/Unihan_Readings.txt
python3 tools/lexicon/test_supplement.py
```

工具先校验固定源文件 SHA256，不联网，不执行下载内容，不调用云模型。源文件哈希不一致时拒绝生成。生成过程不读取评测答案；文章整句答案没有写入补充库。

新增来源以后必须重新核对许可和读音，不能直接把分词词表当成拼音词库。测试分别记录完整词条覆盖、实际首选、候选位置和原有输入的退步情况。
