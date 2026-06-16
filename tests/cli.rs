//! 端到端集成测试：用 fixtures/ 里的小夹具实际跑 info / verify / extract。
//!
//! 仅在 Windows 上编译运行——整个 crate 依赖 wimlib_tchar=wchar_t 与
//! libwim-15.dll（由 build.rs 复制到二进制同目录），非 Windows 平台不适用。
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// 由 cargo 注入的被测二进制路径（其同目录已被 build.rs 放入 libwim-15.dll）。
fn imgtool() -> Command {
    Command::new(env!("CARGO_BIN_EXE_imgtool"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

/// 进程级唯一的临时输出目录，避免并发测试相互踩踏。
fn unique_out(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("imgtool_it_{tag}_{}_{nanos}", std::process::id()))
}

fn count_files(dir: &Path) -> usize {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                n += count_files(&p);
            } else {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn info_reports_two_images() {
    let out = imgtool()
        .arg("info")
        .arg(fixture("test.wim"))
        .output()
        .expect("运行 info 失败");
    assert!(out.status.success(), "info 退出码非 0: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let two = stdout.lines().any(|l| l.contains("卷数") && l.contains('2'));
    assert!(two, "info 应报告 卷数=2，实际输出:\n{stdout}");
    // 修复后 version_string() 必须正确解码 UTF-16，info 应打印 wimlib 版本行。
    assert!(stdout.contains("wimlib"), "info 应打印 wimlib 版本行:\n{stdout}");
}

#[test]
fn verify_good_wim_succeeds() {
    let status = imgtool()
        .arg("verify")
        .arg(fixture("test.wim"))
        .status()
        .expect("运行 verify 失败");
    assert!(status.success(), "verify test.wim 应通过，实际退出码 {:?}", status.code());
}

#[test]
fn verify_corrupt_wim_fails_with_code_2() {
    let status = imgtool()
        .arg("verify")
        .arg(fixture("corrupt.wim"))
        .status()
        .expect("运行 verify 失败");
    assert_eq!(status.code(), Some(2), "corrupt.wim 应以退出码 2（损坏）失败");
}

#[test]
fn extract_wim_index1() {
    let dest = unique_out("wim");
    let status = imgtool()
        .args(["extract"])
        .arg(fixture("test.wim"))
        .args(["--index", "1", "--dest"])
        .arg(&dest)
        .status()
        .expect("运行 extract 失败");
    let n = count_files(&dest);
    let _ = std::fs::remove_dir_all(&dest);
    assert!(status.success(), "extract WIM 退出码非 0");
    assert!(n > 0, "extract WIM 未解出任何文件");
}

#[test]
fn extract_esd_index1() {
    let dest = unique_out("esd");
    let status = imgtool()
        .args(["extract"])
        .arg(fixture("test.esd"))
        .args(["--index", "1", "--dest"])
        .arg(&dest)
        .status()
        .expect("运行 extract 失败");
    let n = count_files(&dest);
    let _ = std::fs::remove_dir_all(&dest);
    assert!(status.success(), "extract ESD 退出码非 0（ESD 通路）");
    assert!(n > 0, "extract ESD 未解出任何文件");
}

#[test]
fn extract_swm_auto_merge() {
    let dest = unique_out("swm");
    // 只传第一片，程序应自动 glob 引用 test2.swm / test3.swm 并合并解出。
    let status = imgtool()
        .args(["extract"])
        .arg(fixture("test.swm"))
        .args(["--dest"])
        .arg(&dest)
        .status()
        .expect("运行 extract 失败");
    let n = count_files(&dest);
    let _ = std::fs::remove_dir_all(&dest);
    assert!(status.success(), "extract SWM 退出码非 0");
    assert!(n > 0, "extract SWM 未解出任何文件");
}
