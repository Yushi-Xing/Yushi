# 本分支的补充词库

基础词库、领域词库及释义表已随上游迁到 `qingjian-team/qingjian-dicts`，通过 `tools/release/dicts.lock` 下载。本分支另按 `dicts-extra.lock` 保留随包通用扩展词库，来源与许可见 [GENERAL-NOTICE.md](GENERAL-NOTICE.md)。这里只保留本分支编译内嵌的读音补丁、常用补充库及其来源许可。

`dict.tsv`、`dicts/`、`03_domains/` 和 `01_characters/standard_8105.tsv` 是生成本 fork 补充词库所用的冻结来源，只用于来源核验与离线复现，不参与当前主词库打包。哈希固定在 `supplement/manifest.json`。

## 基础读音补丁

`patches.tsv` 是本项目手工维护的基础读音与排序权重，按仓库许可证分发，权重不代表实测频次。Windows 与 CLI 将它作为编译内嵌的附加词库使用，与锁定的产品数据归档分开，词库热加载时保留。当前补充 `en` 对应的“嗯”（上游 #300），以及“环太平洋”等 7 个常用作品名与“任务栏、候选词、自动纠错”等 8 个常用词；不包含评测标准句。专名与通用词的 2000／3000 权重均为保守的人工排序值。

## 常用补充库

[supplement/](supplement/README.md) 补入 19,662 个常用词形，来源为 MIT 许可的结巴与 THUOCL，读音采用已核验整词读音或 Unicode 17.0.0 单音字。Windows 与 CLI 默认生效；完整来源、许可、筛选与复现命令见该目录。
