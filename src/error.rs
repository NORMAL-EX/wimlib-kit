//! 错误类型定义与 wimlib 错误码映射。

use std::os::raw::c_int;

use crate::ffi::types::WIMLIB_ERR_INTEGRITY;
use crate::ffi::WimlibApi;
use crate::util::from_wide;

#[derive(thiserror::Error, Debug)]
pub enum WimError {
    #[error("未找到 {name}，请确认它与程序在同一目录（或在系统库搜索路径中）。底层错误: {inner:?}")]
    DllNotFound {
        name: String,
        inner: Option<libloading::Error>,
    },

    #[error("无法从 DLL 解析符号 {name}")]
    SymbolNotFound {
        name: String,
        #[source]
        source: libloading::Error,
    },

    /// wimlib 返回的非零错误码。`code` 为原始错误码，`message` 为 wimlib 给出的描述。
    #[error("wimlib 错误[{code}]: {message}")]
    Wimlib { code: i32, message: String },

    #[error("路径无效或包含无法处理的字符: {0}")]
    InvalidPath(String),

    #[error("{0}")]
    Other(String),
}

impl WimError {
    /// 该错误是否表示“完整性校验失败 / 镜像损坏”（错误码 13）。
    pub fn is_integrity_failure(&self) -> bool {
        matches!(self, WimError::Wimlib { code, .. } if *code == WIMLIB_ERR_INTEGRITY)
    }

    /// 用已加载的 API 把错误码转成带描述的 `WimError::Wimlib`。
    pub fn from_code_with_api(code: c_int, api: &WimlibApi) -> Self {
        let msg = unsafe {
            let ptr = (api.get_error_string)(code);
            from_wide(ptr).unwrap_or_else(|| format!("错误码 {code}"))
        };
        WimError::Wimlib {
            code: code as i32,
            message: msg,
        }
    }
}
