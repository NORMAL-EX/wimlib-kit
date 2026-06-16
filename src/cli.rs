//! clap 子命令定义与各功能实现：info / verify / extract。

use std::path::Path;

use clap::{Parser, Subcommand};

use crate::error::WimError;
use crate::ffi::types::*;
use crate::ffi::WimlibApi;
use crate::progress::{ProgressKind, ProgressState};
use crate::util::human_bytes;
use crate::wim::Wim;

#[derive(Parser)]
#[command(
    name = "imgtool",
    version,
    about = "基于 wimlib 的 Windows 镜像工具（WIM/ESD/SWM）：信息 / 校验 / 解包"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// 读取并打印镜像信息（卷数、卷名、版本、压缩方式、各卷大小）
    Info {
        /// 镜像路径（.wim/.esd/.swm）
        image: String,
    },
    /// 校验镜像完整性（带进度条，损坏时退出码非 0）
    Verify {
        /// 镜像路径
        image: String,
    },
    /// 解包/应用镜像到目录（带进度条）
    Extract {
        /// 镜像路径（SWM 传第一片即可，自动合并同目录其余分卷）
        image: String,
        /// 卷号（1 起始）或 all 表示全部
        #[arg(long, default_value = "1")]
        index: String,
        /// 目标目录
        #[arg(long)]
        dest: String,
    },
    /// ESD↔WIM 转换 / 重压缩（把所有卷导出到新容器，带进度条）
    Convert {
        /// 源镜像（.wim/.esd）
        image: String,
        /// 输出文件路径（按扩展名 .wim/.esd 推断目标类型，可被 --to 覆盖）
        #[arg(long)]
        dest: String,
        /// 目标容器类型：wim 或 esd（默认按 dest 扩展名推断）
        #[arg(long)]
        to: Option<String>,
        /// 压缩算法：none|xpress|lzx|lzms（默认 wim→lzx、esd→lzms）
        #[arg(long)]
        compress: Option<String>,
        /// 写出后写入完整性表
        #[arg(long)]
        check: bool,
    },
    /// 制作镜像：把一个目录捕获打包成 WIM/ESD（带进度条）
    Capture {
        /// 源目录
        source: String,
        /// 输出文件路径（按扩展名 .wim/.esd 推断目标类型，可被 --to 覆盖）
        #[arg(long)]
        dest: String,
        /// 卷名（写入镜像元数据）
        #[arg(long)]
        name: Option<String>,
        /// 目标容器类型：wim 或 esd（默认按 dest 扩展名推断）
        #[arg(long)]
        to: Option<String>,
        /// 压缩算法：none|xpress|lzx|lzms（默认 wim→lzx、esd→lzms）
        #[arg(long)]
        compress: Option<String>,
        /// 写出后写入完整性表
        #[arg(long)]
        check: bool,
    },
    /// 分卷：把镜像分割为多个 SWM 分卷（带进度条）
    Split {
        /// 源镜像（.wim/.esd）
        image: String,
        /// 输出 SWM 第一片路径（如 out.swm，后续片自动编号）
        #[arg(long)]
        dest: String,
        /// 每片最大大小（MiB）
        #[arg(long, default_value = "1024")]
        size: u64,
        /// 写出后写入完整性表
        #[arg(long)]
        check: bool,
    },
    /// 合并：把 SWM 分卷合并回一个 WIM（传任一分卷，自动查找其余片）
    Join {
        /// 任一 SWM 分卷路径
        image: String,
        /// 输出 WIM 文件
        #[arg(long)]
        dest: String,
        /// 写出后写入完整性表
        #[arg(long)]
        check: bool,
    },
    /// 优化：原地重写镜像，重建并（可选）重压缩以瘦身（带进度条）
    Optimize {
        /// 镜像路径（将被原地重写）
        image: String,
        /// 强制重新压缩已压缩的数据（更慢，体积更小）
        #[arg(long)]
        recompress: bool,
        /// 写入完整性表
        #[arg(long)]
        check: bool,
    },
}

/// 校验失败（镜像损坏）时使用的退出码，区别于一般错误。
pub const EXIT_CORRUPT: i32 = 2;

pub fn run(cli: Cli, api: &WimlibApi) -> Result<(), WimError> {
    match cli.command {
        Command::Info { image } => cmd_info(api, &image),
        Command::Verify { image } => cmd_verify(api, &image),
        Command::Extract { image, index, dest } => cmd_extract(api, &image, &index, &dest),
        Command::Convert {
            image,
            dest,
            to,
            compress,
            check,
        } => cmd_convert(api, &image, &dest, to.as_deref(), compress.as_deref(), check),
        Command::Capture {
            source,
            dest,
            name,
            to,
            compress,
            check,
        } => cmd_capture(
            api,
            &source,
            &dest,
            name.as_deref(),
            to.as_deref(),
            compress.as_deref(),
            check,
        ),
        Command::Split {
            image,
            dest,
            size,
            check,
        } => cmd_split(api, &image, &dest, size, check),
        Command::Join { image, dest, check } => cmd_join(api, &image, &dest, check),
        Command::Optimize {
            image,
            recompress,
            check,
        } => cmd_optimize(api, &image, recompress, check),
    }
}

fn cmd_info(api: &WimlibApi, image: &str) -> Result<(), WimError> {
    let wim = Wim::open(api, image, 0, std::ptr::null_mut())?;
    // SWM 第一片只含部分资源，但 get_wim_info 仍能给出总卷数等信息。
    let info = wim.info()?;

    println!("wimlib   : {}", api.version_string());
    println!("镜像文件 : {image}");
    println!("卷数     : {}", info.image_count);
    println!("WIM 版本 : 0x{:08x}", info.wim_version);
    println!("压缩方式 : {}", compression_type_name(info.compression_type));
    println!("块大小   : {}", human_bytes(info.chunk_size as u64));
    if info.total_parts > 1 {
        println!("分卷     : 第 {}/{} 片", info.part_number, info.total_parts);
    }
    println!("完整性表 : {}", if info.has_integrity_table() { "有" } else { "无" });
    println!("本卷大小 : {}", human_bytes(info.total_bytes));
    println!();

    for i in 1..=info.image_count {
        println!("── 卷 {i} ──");
        if let Some(name) = wim.image_property(i, "NAME") {
            println!("  名称       : {name}");
        }
        if let Some(disp) = wim.image_property(i, "DISPLAYNAME") {
            println!("  显示名     : {disp}");
        }
        if let Some(desc) = wim.image_property(i, "DESCRIPTION") {
            println!("  描述       : {desc}");
        }
        if let Some(product) = wim.image_property(i, "WINDOWS/PRODUCTNAME") {
            println!("  产品       : {product}");
        }
        if let Some(flags) = wim.image_property(i, "FLAGS") {
            println!("  FLAGS      : {flags}");
        }
        if let Some(tb) = wim.image_property(i, "TOTALBYTES") {
            if let Ok(bytes) = tb.parse::<u64>() {
                println!("  数据大小   : {}", human_bytes(bytes));
            }
        }
    }
    Ok(())
}

fn cmd_verify(api: &WimlibApi, image: &str) -> Result<(), WimError> {
    let mut state = ProgressState::new(ProgressKind::Verify);
    let ctx = &mut state as *mut ProgressState as *mut std::os::raw::c_void;

    // 用 CHECK_INTEGRITY 打开：打开过程即触发完整性校验与进度回调。
    let result = Wim::open(api, image, WIMLIB_OPEN_FLAG_CHECK_INTEGRITY, ctx);

    match result {
        Ok(wim) => {
            state.finish(true);
            let info = wim.info()?;
            if info.has_integrity_table() {
                println!("校验通过：镜像完整性 OK");
            } else {
                println!("⚠ 该镜像不含完整性表，已跳过哈希校验（无法判定是否损坏）。");
            }
            Ok(())
        }
        Err(e) => {
            state.finish(false);
            if e.is_integrity_failure() {
                eprintln!("校验失败：镜像已损坏（完整性校验未通过，错误码 13）");
            }
            Err(e)
        }
    }
}

/// 根据 SWM 第一片路径构造引用其余分卷的 glob，例如 dir/test.swm -> dir/test*.swm。
fn make_swm_glob(path: &str) -> String {
    let p = Path::new(path);
    let dir = p.parent().filter(|d| !d.as_os_str().is_empty());
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    // 去掉结尾数字，得到分卷公共前缀（test1/test2... -> test）。
    let base: String = stem.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
    let base = if base.is_empty() { stem.to_string() } else { base };
    let pattern = format!("{base}*.swm");
    match dir {
        Some(d) => d.join(pattern).to_string_lossy().into_owned(),
        None => pattern,
    }
}

fn cmd_extract(api: &WimlibApi, image: &str, index: &str, dest: &str) -> Result<(), WimError> {
    let image_index = if index.eq_ignore_ascii_case("all") {
        WIMLIB_ALL_IMAGES
    } else {
        index
            .parse::<i32>()
            .map_err(|_| WimError::Other(format!("无效的卷号: {index}（应为正整数或 all）")))?
    };

    std::fs::create_dir_all(dest)
        .map_err(|e| WimError::Other(format!("无法创建目标目录 {dest}: {e}")))?;

    let is_swm = Path::new(image)
        .extension()
        .map(|e| e.eq_ignore_ascii_case("swm"))
        .unwrap_or(false);

    let mut state = ProgressState::new(ProgressKind::Extract);
    let ctx = &mut state as *mut ProgressState as *mut std::os::raw::c_void;

    let wim = Wim::open(api, image, 0, ctx)?;

    if is_swm {
        let glob = make_swm_glob(image);
        println!("检测到 SWM 分卷，引用其余分片: {glob}");
        wim.reference_glob(&glob)?;
    }

    let result = wim.extract(image_index, dest);
    match &result {
        Ok(()) => {
            state.finish(true);
            println!("解包完成 -> {dest}");
        }
        Err(_) => state.finish(false),
    }
    result
}

/// 解析压缩类型名（none|xpress|lzx|lzms）。
fn parse_compression(name: &str) -> Result<i32, WimError> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "none" => WIMLIB_COMPRESSION_TYPE_NONE,
        "xpress" => WIMLIB_COMPRESSION_TYPE_XPRESS,
        "lzx" => WIMLIB_COMPRESSION_TYPE_LZX,
        "lzms" => WIMLIB_COMPRESSION_TYPE_LZMS,
        other => {
            return Err(WimError::Other(format!(
                "未知压缩类型: {other}（可选 none|xpress|lzx|lzms）"
            )))
        }
    })
}

/// 根据 --to / --compress 与输出扩展名，解析目标压缩类型、是否 solid、是否 ESD 容器。
fn resolve_target(
    dest: &str,
    to: Option<&str>,
    compress: Option<&str>,
) -> Result<(i32, bool, bool), WimError> {
    // 目标容器类型：显式 --to 优先，否则按输出扩展名推断（.esd→esd，其余→wim）。
    let to_esd = match to {
        Some(t) if t.eq_ignore_ascii_case("esd") => true,
        Some(t) if t.eq_ignore_ascii_case("wim") => false,
        Some(other) => {
            return Err(WimError::Other(format!(
                "未知目标类型: {other}（可选 wim|esd）"
            )))
        }
        None => Path::new(dest)
            .extension()
            .map(|e| e.eq_ignore_ascii_case("esd"))
            .unwrap_or(false),
    };
    // 压缩算法：显式 --compress 优先，否则 esd→LZMS(solid)、wim→LZX。
    let (ctype, solid) = match compress {
        Some(c) => {
            let ct = parse_compression(c)?;
            (ct, ct == WIMLIB_COMPRESSION_TYPE_LZMS)
        }
        None if to_esd => (WIMLIB_COMPRESSION_TYPE_LZMS, true),
        None => (WIMLIB_COMPRESSION_TYPE_LZX, false),
    };
    Ok((ctype, solid, to_esd))
}

fn cmd_convert(
    api: &WimlibApi,
    image: &str,
    dest: &str,
    to: Option<&str>,
    compress: Option<&str>,
    check: bool,
) -> Result<(), WimError> {
    let (ctype, solid, to_esd) = resolve_target(dest, to, compress)?;

    let mut state = ProgressState::new(ProgressKind::Convert);
    let ctx = &mut state as *mut ProgressState as *mut std::os::raw::c_void;

    // 源镜像只读打开（无需进度）；目标新建后承载写出进度。
    let src = Wim::open(api, image, 0, std::ptr::null_mut())?;
    let dest_wim = Wim::create_new(api, ctype)?;
    src.export_to(&dest_wim, WIMLIB_ALL_IMAGES)?;
    dest_wim.set_output_compression(ctype)?;
    dest_wim.register_progress(ctx);

    let mut write_flags = WIMLIB_WRITE_FLAG_REBUILD;
    if solid {
        write_flags |= WIMLIB_WRITE_FLAG_SOLID;
    }
    if check {
        write_flags |= WIMLIB_WRITE_FLAG_CHECK_INTEGRITY;
    }

    println!(
        "转换 {image} -> {dest}（{}，{}）",
        if to_esd { "ESD" } else { "WIM" },
        compression_type_name(ctype)
    );
    let result = dest_wim.write_to(dest, write_flags);
    match &result {
        Ok(()) => {
            state.finish(true);
            println!("转换完成 -> {dest}");
        }
        Err(_) => state.finish(false),
    }
    result
}

fn cmd_capture(
    api: &WimlibApi,
    source: &str,
    dest: &str,
    name: Option<&str>,
    to: Option<&str>,
    compress: Option<&str>,
    check: bool,
) -> Result<(), WimError> {
    let (ctype, solid, to_esd) = resolve_target(dest, to, compress)?;

    let mut state = ProgressState::new(ProgressKind::Capture);
    let ctx = &mut state as *mut ProgressState as *mut std::os::raw::c_void;

    let wim = Wim::create_new(api, ctype)?;
    println!("正在扫描并捕获目录 {source} ...");
    wim.add_image(source, name)?;
    wim.set_output_compression(ctype)?;
    wim.register_progress(ctx);

    let mut write_flags = WIMLIB_WRITE_FLAG_REBUILD;
    if solid {
        write_flags |= WIMLIB_WRITE_FLAG_SOLID;
    }
    if check {
        write_flags |= WIMLIB_WRITE_FLAG_CHECK_INTEGRITY;
    }

    println!(
        "制作 {source} -> {dest}（{}，{}）",
        if to_esd { "ESD" } else { "WIM" },
        compression_type_name(ctype)
    );
    let result = wim.write_to(dest, write_flags);
    match &result {
        Ok(()) => {
            state.finish(true);
            println!("制作完成 -> {dest}");
        }
        Err(_) => state.finish(false),
    }
    result
}

fn cmd_split(
    api: &WimlibApi,
    image: &str,
    dest: &str,
    size_mib: u64,
    check: bool,
) -> Result<(), WimError> {
    let part_size = size_mib.max(1).saturating_mul(1024 * 1024);

    let mut state = ProgressState::new(ProgressKind::Split);
    let ctx = &mut state as *mut ProgressState as *mut std::os::raw::c_void;

    let wim = Wim::open(api, image, 0, ctx)?;
    let write_flags = if check {
        WIMLIB_WRITE_FLAG_CHECK_INTEGRITY
    } else {
        0
    };

    println!("分卷 {image} -> {dest}（每片 ≤ {size_mib} MiB）");
    let result = wim.split(dest, part_size, write_flags);
    match &result {
        Ok(()) => {
            state.finish(true);
            println!("分卷完成 -> {dest}");
        }
        Err(_) => state.finish(false),
    }
    result
}

fn cmd_join(api: &WimlibApi, image: &str, dest: &str, check: bool) -> Result<(), WimError> {
    let parts = swm_parts(image)?;

    let mut state = ProgressState::new(ProgressKind::Join);
    let ctx = &mut state as *mut ProgressState as *mut std::os::raw::c_void;
    let write_flags = if check {
        WIMLIB_WRITE_FLAG_CHECK_INTEGRITY
    } else {
        0
    };

    println!("合并 {} 个分卷 -> {dest}", parts.len());
    let result = Wim::join_swms(api, &parts, dest, write_flags, ctx);
    match &result {
        Ok(()) => {
            state.finish(true);
            println!("合并完成 -> {dest}");
        }
        Err(_) => state.finish(false),
    }
    result
}

/// 根据任一 SWM 分卷路径，查找同目录下同前缀的全部 .swm 分卷（用于 join）。
fn swm_parts(first: &str) -> Result<Vec<String>, WimError> {
    let p = Path::new(first);
    let dir = p.parent().filter(|d| !d.as_os_str().is_empty());
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let base = stem.trim_end_matches(|c: char| c.is_ascii_digit());
    let base = if base.is_empty() { stem } else { base };
    let dir_path = dir.unwrap_or_else(|| Path::new("."));

    let mut parts = Vec::new();
    let rd = std::fs::read_dir(dir_path)
        .map_err(|e| WimError::Other(format!("读取目录 {} 失败: {e}", dir_path.display())))?;
    for entry in rd {
        let entry = entry.map_err(|e| WimError::Other(format!("读取目录项失败: {e}")))?;
        let fname = entry.file_name();
        let name = fname.to_string_lossy();
        if name.starts_with(base) && name.to_ascii_lowercase().ends_with(".swm") {
            parts.push(entry.path().to_string_lossy().into_owned());
        }
    }
    parts.sort();
    if parts.is_empty() {
        return Err(WimError::Other(format!(
            "未找到任何 SWM 分卷（目录 {}，前缀 {base}）",
            dir_path.display()
        )));
    }
    Ok(parts)
}

fn cmd_optimize(
    api: &WimlibApi,
    image: &str,
    recompress: bool,
    check: bool,
) -> Result<(), WimError> {
    let mut state = ProgressState::new(ProgressKind::Optimize);
    let ctx = &mut state as *mut ProgressState as *mut std::os::raw::c_void;

    let wim = Wim::open(api, image, 0, ctx)?;
    let mut write_flags = WIMLIB_WRITE_FLAG_REBUILD;
    if recompress {
        write_flags |= WIMLIB_WRITE_FLAG_RECOMPRESS;
    }
    if check {
        write_flags |= WIMLIB_WRITE_FLAG_CHECK_INTEGRITY;
    }

    println!("优化（原地重写）{image} ...");
    let result = wim.overwrite(write_flags);
    match &result {
        Ok(()) => {
            state.finish(true);
            println!("优化完成 -> {image}");
        }
        Err(_) => state.finish(false),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::make_swm_glob;

    #[test]
    fn swm_glob_plain() {
        assert_eq!(make_swm_glob("test.swm"), "test*.swm");
    }

    // test.swm / test2.swm / test3.swm 三片应归并到同一个 glob。
    #[test]
    fn swm_glob_strips_trailing_digits() {
        assert_eq!(make_swm_glob("test3.swm"), "test*.swm");
    }

    // stem 全是数字时不能 trim 成空，应原样保留。
    #[test]
    fn swm_glob_all_digit_stem_kept() {
        assert_eq!(make_swm_glob("123.swm"), "123*.swm");
    }

    #[test]
    fn swm_glob_preserves_directory() {
        let g = make_swm_glob("fixtures/test.swm");
        assert!(g.ends_with("test*.swm"), "实际: {g}");
        assert!(g.contains("fixtures"), "实际: {g}");
    }
}
