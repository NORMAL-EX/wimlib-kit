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
}

/// 校验失败（镜像损坏）时使用的退出码，区别于一般错误。
pub const EXIT_CORRUPT: i32 = 2;

pub fn run(cli: Cli, api: &WimlibApi) -> Result<(), WimError> {
    match cli.command {
        Command::Info { image } => cmd_info(api, &image),
        Command::Verify { image } => cmd_verify(api, &image),
        Command::Extract { image, index, dest } => cmd_extract(api, &image, &index, &dest),
    }
}

fn cmd_info(api: &WimlibApi, image: &str) -> Result<(), WimError> {
    let wim = Wim::open(api, image, 0, std::ptr::null_mut())?;
    // SWM 第一片只含部分资源，但 get_wim_info 仍能给出总卷数等信息。
    let info = wim.info()?;

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
