use std::env;
use std::fs;
use std::path::Path;

// 把 vendor/libwim-15.dll 复制到最终可执行文件所在目录（target/<profile>/），
// 以便 libloading 在运行时能在 exe 同目录找到它。
fn main() {
    let dll_name = "libwim-15.dll";
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 未设置");
    let src = Path::new(&manifest_dir).join("vendor").join(dll_name);

    println!("cargo:rerun-if-changed=vendor/{dll_name}");

    if !src.exists() {
        // 非 Windows 或缺少 DLL 时不阻断编译，仅提示。
        println!("cargo:warning=未找到 {}，跳过 DLL 复制", src.display());
        return;
    }

    // OUT_DIR 形如 target/<profile>/build/<pkg-hash>/out，向上 3 级即 target/<profile>。
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR 未设置");
    let target_dir = Path::new(&out_dir)
        .ancestors()
        .nth(3)
        .expect("无法从 OUT_DIR 推断 target 目录");

    let dst = target_dir.join(dll_name);
    if let Err(e) = fs::copy(&src, &dst) {
        println!("cargo:warning=复制 DLL 失败 {} -> {}: {e}", src.display(), dst.display());
    }

    // 同时复制一份到 deps 目录，方便 cargo test 直接运行测试二进制时定位。
    let deps_dir = target_dir.join("deps");
    if deps_dir.exists() {
        let _ = fs::copy(&src, deps_dir.join(dll_name));
    }
}
