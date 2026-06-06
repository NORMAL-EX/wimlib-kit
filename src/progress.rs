//! indicatif 进度条封装 + 作为 progctx 传给 wimlib 回调的状态结构。
//!
//! 速度与 ETA 直接交给 indicatif 计算（基于 set_position 的时间序列），
//! wimlib 只提供 completed_bytes / total_bytes。

use indicatif::{ProgressBar, ProgressStyle};

/// 进度任务类型，用于设定进度条前缀文案。
#[derive(Clone, Copy)]
pub enum ProgressKind {
    Verify,
    Extract,
}

impl ProgressKind {
    fn prefix(&self) -> &'static str {
        match self {
            ProgressKind::Verify => "校验",
            ProgressKind::Extract => "解包",
        }
    }
}

/// 通过 progctx 指针在 FFI 回调里取回的状态。
pub struct ProgressState {
    bar: ProgressBar,
    kind: ProgressKind,
    length_set: bool,
}

impl ProgressState {
    pub fn new(kind: ProgressKind) -> Self {
        let bar = ProgressBar::new(0);
        bar.set_style(
            ProgressStyle::with_template(
                "{prefix:<4} [{bar:32.cyan/blue}] {percent:>3}% {bytes}/{total_bytes} {binary_bytes_per_sec} ETA {eta}",
            )
            .unwrap()
            .progress_chars("=>-"),
        );
        bar.set_prefix(kind.prefix());
        ProgressState {
            bar,
            kind,
            length_set: false,
        }
    }

    /// 收到字节进度时更新进度条；首次会设置总长度。
    pub fn update(&mut self, completed: u64, total: u64) {
        if !self.length_set && total > 0 {
            self.bar.set_length(total);
            self.length_set = true;
        }
        self.bar.set_position(completed);
    }

    pub fn set_stage(&mut self, stage: &str) {
        self.bar
            .set_prefix(format!("{}·{}", self.kind.prefix(), stage));
    }

    /// 操作完成，定格进度条。
    pub fn finish(&self, ok: bool) {
        if ok {
            self.bar.finish();
        } else {
            self.bar.abandon();
        }
    }
}
