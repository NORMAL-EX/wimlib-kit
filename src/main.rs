mod callback;
mod cli;
mod error;
mod ffi;
mod progress;
mod util;
mod wim;

use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, EXIT_CORRUPT};
use ffi::WimlibApi;

fn main() -> ExitCode {
    let cli = Cli::parse();

    // 加载 DLL + 全局初始化。
    let api = match WimlibApi::load() {
        Ok(api) => api,
        Err(e) => {
            eprintln!("错误: {e}");
            return ExitCode::FAILURE;
        }
    };

    match cli::run(cli, &api) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // 镜像损坏用专门的退出码，便于脚本判定。
            if e.is_integrity_failure() {
                return ExitCode::from(EXIT_CORRUPT as u8);
            }
            eprintln!("错误: {e}");
            ExitCode::FAILURE
        }
    }
}
