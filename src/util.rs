//! 宽字符转换辅助：所有 UTF-8 <-> UTF-16 的转换都集中在此处。
//!
//! Windows 上 wimlib_tchar = wchar_t = 2 字节 UTF-16LE，所有传给 wimlib 的路径
//! 都必须是 NUL 结尾的 *const u16，绝不能直接传 Rust 的 UTF-8 字节。

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

/// 把 &str 转成 NUL 结尾的 UTF-16 缓冲区。
pub fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// 从 wimlib 返回的（可能为空的）NUL 结尾 UTF-16 指针读出 String。
///
/// # Safety
/// `ptr` 必须为 NULL 或指向一段合法、以 0 结尾的 UTF-16 序列。
pub unsafe fn from_wide(ptr: *const u16) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    let slice = std::slice::from_raw_parts(ptr, len);
    Some(String::from_utf16_lossy(slice))
}

/// 以易读单位格式化字节数。
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let mut val = bytes as f64;
    let mut idx = 0;
    while val >= 1024.0 && idx < UNITS.len() - 1 {
        val /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{val:.2} {}", UNITS[idx])
    }
}
