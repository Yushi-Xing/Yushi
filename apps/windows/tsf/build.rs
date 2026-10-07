//! MSVC 测试宿主声明 Windows 8+ 兼容性，才能验证分层子窗口。

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=resources/hosted-tests.manifest");
    if env::var("TARGET").is_ok_and(|target| target.ends_with("windows-msvc")) {
        let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("crate directory"))
            .join("resources/hosted-tests.manifest");
        // unit test 仍是 lib target，rustc-link-arg-tests 不会覆盖它。固定 ID 1 是 exe
        // 清单，产品 DLL 加载时不会用它覆盖 SearchHost 的应用兼容性或权限。
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED,ID=1");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFESTUAC:NO");
    }
}
