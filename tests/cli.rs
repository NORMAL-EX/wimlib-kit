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

#[test]
fn convert_esd_to_wim() {
    let dest = unique_out("conv").with_extension("wim");
    let status = imgtool()
        .args(["convert"])
        .arg(fixture("test.esd"))
        .args(["--to", "wim", "--dest"])
        .arg(&dest)
        .status()
        .expect("运行 convert 失败");
    assert!(status.success(), "convert ESD→WIM 退出码非 0");
    assert!(dest.is_file(), "convert 未产出目标文件");

    // 转出的 WIM 应能被正确读取：2 卷、压缩为 LZX。
    let out = imgtool()
        .arg("info")
        .arg(&dest)
        .output()
        .expect("运行 info 失败");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_file(&dest);
    assert!(out.status.success(), "info 读取转出的 WIM 失败");
    assert!(
        stdout.lines().any(|l| l.contains("卷数") && l.contains('2')),
        "转出的 WIM 卷数应为 2:\n{stdout}"
    );
    assert!(stdout.contains("LZX"), "转出的 WIM 压缩应为 LZX:\n{stdout}");
}

#[test]
fn capture_dir_to_wim() {
    // 准备一个临时源目录并放入一个文件。
    let src = unique_out("capsrc");
    std::fs::create_dir_all(&src).expect("建源目录失败");
    std::fs::write(src.join("hello.txt"), b"wimlib-kit capture test").expect("写测试文件失败");
    let dest = unique_out("cap").with_extension("wim");

    let status = imgtool()
        .args(["capture"])
        .arg(&src)
        .args(["--name", "TestImg", "--dest"])
        .arg(&dest)
        .status()
        .expect("运行 capture 失败");
    let _ = std::fs::remove_dir_all(&src);
    assert!(status.success(), "capture 退出码非 0");
    assert!(dest.is_file(), "capture 未产出镜像文件");

    // 制成的 WIM 应有 1 卷，且卷名为 TestImg。
    let out = imgtool()
        .arg("info")
        .arg(&dest)
        .output()
        .expect("运行 info 失败");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_file(&dest);
    assert!(out.status.success(), "info 读取制成的 WIM 失败");
    assert!(
        stdout.lines().any(|l| l.contains("卷数") && l.contains('1')),
        "制成的 WIM 应有 1 卷:\n{stdout}"
    );
    assert!(stdout.contains("TestImg"), "应包含卷名 TestImg:\n{stdout}");
}

#[test]
fn split_and_join_roundtrip() {
    let dir = unique_out("splitdir");
    std::fs::create_dir_all(&dir).expect("建分卷目录失败");
    let swm = dir.join("part.swm");

    // 分卷（每片 1 MiB；夹具较小可能只 1 片，但 round-trip 仍验证正确性）。
    let status = imgtool()
        .args(["split"])
        .arg(fixture("test.wim"))
        .arg("--dest")
        .arg(&swm)
        .args(["--size", "1"])
        .status()
        .expect("运行 split 失败");
    assert!(status.success(), "split 退出码非 0");
    let part_count = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |x| x.eq_ignore_ascii_case("swm"))
        })
        .count();
    assert!(part_count > 0, "split 未产出任何 SWM 分卷");

    // 合并回 WIM（传第一片，自动查找其余片）。
    let joined = unique_out("joined").with_extension("wim");
    let status2 = imgtool()
        .args(["join"])
        .arg(&swm)
        .arg("--dest")
        .arg(&joined)
        .status()
        .expect("运行 join 失败");
    assert!(status2.success(), "join 退出码非 0");

    // 合并后的 WIM 应仍有 2 卷。
    let out = imgtool()
        .arg("info")
        .arg(&joined)
        .output()
        .expect("运行 info 失败");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&joined);
    assert!(out.status.success(), "info 读取合并后的 WIM 失败");
    assert!(
        stdout.lines().any(|l| l.contains("卷数") && l.contains('2')),
        "合并后的 WIM 卷数应为 2:\n{stdout}"
    );
}

#[test]
fn optimize_inplace() {
    // 复制夹具到临时文件再原地优化，避免改动仓库里的 fixtures。
    let work = unique_out("opt").with_extension("wim");
    std::fs::copy(fixture("test.wim"), &work).expect("复制夹具失败");

    let status = imgtool()
        .args(["optimize"])
        .arg(&work)
        .arg("--recompress")
        .status()
        .expect("运行 optimize 失败");
    assert!(status.success(), "optimize 退出码非 0");

    // 优化后仍应是有效 WIM、2 卷。
    let out = imgtool()
        .arg("info")
        .arg(&work)
        .output()
        .expect("运行 info 失败");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_file(&work);
    assert!(out.status.success(), "info 读取优化后的 WIM 失败");
    assert!(
        stdout.lines().any(|l| l.contains("卷数") && l.contains('2')),
        "优化后的 WIM 卷数应为 2:\n{stdout}"
    );
}

#[test]
fn export_single_image_to_new_wim() {
    let dest = unique_out("exp").with_extension("wim");
    let status = imgtool()
        .args(["export"])
        .arg(fixture("test.wim"))
        .args(["--index", "1", "--dest"])
        .arg(&dest)
        .status()
        .expect("运行 export 失败");
    assert!(status.success(), "export 退出码非 0");
    let out = imgtool()
        .arg("info")
        .arg(&dest)
        .output()
        .expect("运行 info 失败");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_file(&dest);
    assert!(out.status.success(), "info 读取导出的 WIM 失败");
    assert!(
        stdout.lines().any(|l| l.contains("卷数") && l.contains('1')),
        "只导出 1 卷后应为 1 卷:\n{stdout}"
    );
}

#[test]
fn delete_image_inplace() {
    let work = unique_out("del").with_extension("wim");
    std::fs::copy(fixture("test.wim"), &work).expect("复制夹具失败");
    let status = imgtool()
        .args(["delete"])
        .arg(&work)
        .args(["--index", "2"])
        .status()
        .expect("运行 delete 失败");
    assert!(status.success(), "delete 退出码非 0");
    let out = imgtool()
        .arg("info")
        .arg(&work)
        .output()
        .expect("运行 info 失败");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_file(&work);
    assert!(out.status.success(), "info 读取删卷后的 WIM 失败");
    assert!(
        stdout.lines().any(|l| l.contains("卷数") && l.contains('1')),
        "原 2 卷删 1 后应剩 1 卷:\n{stdout}"
    );
}
