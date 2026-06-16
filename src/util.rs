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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_bytes_boundaries() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1), "1 B");
        assert_eq!(human_bytes(1023), "1023 B");
        assert_eq!(human_bytes(1024), "1.00 KiB");
        assert_eq!(human_bytes(1536), "1.50 KiB");
        assert_eq!(human_bytes(1024 * 1024), "1.00 MiB");
        assert_eq!(human_bytes(1024 * 1024 * 1024), "1.00 GiB");
        assert_eq!(human_bytes(1_099_511_627_776), "1.00 TiB");
        assert_eq!(
            human_bytes(3u64 * 1024 * 1024 * 1024 + 512 * 1024 * 1024),
            "3.50 GiB"
        );
    }

    // to_wide / from_wide 互为逆操作，是所有 FFI 路径传参的基础。
    #[test]
    fn wide_roundtrip_ascii() {
        let w = to_wide("test.wim");
        assert_eq!(w.last().copied(), Some(0), "to_wide 必须以 NUL 结尾");
        let back = unsafe { from_wide(w.as_ptr()) };
        assert_eq!(back.as_deref(), Some("test.wim"));
    }

    #[test]
    fn wide_roundtrip_non_ascii() {
        let s = "镜像 テスト.wim";
        let w = to_wide(s);
        let back = unsafe { from_wide(w.as_ptr()) };
        assert_eq!(back.as_deref(), Some(s));
    }

    #[test]
    fn wide_empty_string() {
        let w = to_wide("");
        assert_eq!(w, vec![0u16]);
        let back = unsafe { from_wide(w.as_ptr()) };
        assert_eq!(back.as_deref(), Some(""));
    }

    #[test]
    fn from_wide_null_returns_none() {
        let back = unsafe { from_wide(std::ptr::null()) };
        assert_eq!(back, None);
    }
}
