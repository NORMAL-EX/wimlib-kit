//! extern "C" 进度回调。回调内部必须 catch_unwind，绝不让 panic 跨越 FFI 边界。

use std::os::raw::{c_int, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::ffi::types::*;
use crate::progress::ProgressState;

/// 传给 wimlib 的进度回调。`progctx` 实为 `*mut ProgressState`。
pub extern "C" fn progress_callback(
    msg_type: c_int,
    info: *mut c_void,
    progctx: *mut c_void,
) -> c_int {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if progctx.is_null() {
            return;
        }
        let state = unsafe { &mut *(progctx as *mut ProgressState) };

        match msg_type {
            WIMLIB_PROGRESS_MSG_EXTRACT_STREAMS => {
                if info.is_null() {
                    return;
                }
                let e = unsafe { &*(info as *const ProgressInfoExtract) };
                state.update(e.completed_bytes, e.total_bytes);
            }
            WIMLIB_PROGRESS_MSG_VERIFY_INTEGRITY => {
                if info.is_null() {
                    return;
                }
                let i = unsafe { &*(info as *const ProgressInfoIntegrity) };
                state.update(i.completed_bytes, i.total_bytes);
            }
            WIMLIB_PROGRESS_MSG_EXTRACT_IMAGE_BEGIN => state.set_stage("准备"),
            WIMLIB_PROGRESS_MSG_EXTRACT_FILE_STRUCTURE => state.set_stage("目录结构"),
            WIMLIB_PROGRESS_MSG_EXTRACT_METADATA => state.set_stage("元数据"),
            WIMLIB_PROGRESS_MSG_EXTRACT_SPWM_PART_BEGIN => state.set_stage("分卷"),
            WIMLIB_PROGRESS_MSG_EXTRACT_IMAGE_END => state.set_stage("收尾"),
            _ => {}
        }
    }));

    match result {
        Ok(()) => WIMLIB_PROGRESS_STATUS_CONTINUE,
        // 回调内发生 panic：转成 ABORT，让 wimlib 干净地中止当前操作。
        Err(_) => WIMLIB_PROGRESS_STATUS_ABORT,
    }
}
