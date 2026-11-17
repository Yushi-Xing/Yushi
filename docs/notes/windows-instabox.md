# 在 InstaBox 构建 Windows 测试版

本 fork 使用 InstaBox 的 Linux 机器构建 Windows MSVC 产物，不触发 GitHub Actions。Windows Server、共享设置程序、剪贴板与 64 位 TSF 使用 `x86_64-pc-windows-msvc`；32 位 TSF 使用 `i686-pc-windows-msvc`。

## 工具链

- Rust 1.96.0，安装两个 Windows MSVC target。
- `cargo-xwin` 0.23.1、Clang/LLD/LLVM 19、Ninja；`XWIN_ARCH=x86_64,x86`，SDK 缓存放在 `/data`。
- Inno Setup 7.1.0 x64，通过 Wine 10 运行；Wine 必须同时有 64 位和 32 位支持，Inno 的 64 位安装器也会检查 WOW64。
- 产品词库按 `tools/release/dicts.lock` 与本 fork 的 `dicts-extra.lock`、模型按 `tools/release/models.lock` 下载并核验（`bash tools/release/data-fetch.sh` 同时准备两者）；ONNX Runtime 1.24.4 的 NuGet SHA-256 使用 `apps/windows/installer/fetch-onnxruntime.ps1` 中的锁定值。ONNX 的应用目录同时附带锁定的 Visual C++ Runtime 14.44.35211，不能依赖构建机预装的运行库。

用本地 `instabox login` 登录后，在已有编译机的独立工作目录执行。构建前确认工作区提交与发布版本，禁止覆盖其他任务的源码或 Wine 前缀。

```bash
export XWIN_ARCH=x86_64,x86
export XWIN_CACHE_DIR=/data/xwin-cache
export QINGJIAN_UIACCESS=0
cargo +1.96.0 xwin build --release --locked --target x86_64-pc-windows-msvc \
  -p qingjian-windows-server -p qingjian-windows-tsf \
  -p qingjian-settings -p qingjian-clipboard-app
cargo +1.96.0 xwin build --release --locked --target i686-pc-windows-msvc \
  -p qingjian-windows-tsf
```

`.cargo/config.toml` 已为两个目标开启静态 CRT。将 64 位产物拷贝到安装脚本读取的 `target/release/`，32 位 DLL 保持在其 target 目录。按锁文件下载词库、码表、音标和模型，准备 ONNX DLL 与许可文件后，在隔离的 Wine 前缀中用 ISCC 编译原有 `qingjian.iss`，传入 `AppVersion`、去掉预发布后缀的 `AppVersionNumeric` 和 `IncludeLegacySettings=0`。包体清单由 `tools/release/windows-size.py` 生成。

## 验证与发布记录

检查 EXE/DLL 的 PE 架构、DLL 导出与非系统运行库依赖；对安装后的文件核验哈希。在 Wine 中执行能够运行的启动、渲染与安装卸载检查，并记录不支持的系统接口。测试与输入回放报告、源码提交、构建工具版本、数据锁、安装包 SHA-256 随测试版一起归档。

Wine 不能替代 Windows 10/11 真机验收。TSF 注册、系统输入法切换、UWP/搜索框候选显示、32 位宿主与升级兼容性必须在真实 Windows 上确认；没有验证的项目在发布说明中明确列出。未签名的测试版保持 `uiAccess=0`，仅发布到本 fork 的预发布 Release，不更新上游官网的版本索引。
