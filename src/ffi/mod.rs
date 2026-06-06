//! 运行时动态加载 libwim-15.dll，解析并缓存第一阶段需要的函数指针。
//!
//! 设计：把 `Library` 与从中解析出的裸函数指针一起存放在 `WimlibApi` 中。
//! 函数指针只是地址（Copy），只要 `Library` 不被 drop（即 DLL 保持加载），
//! 这些地址就始终有效，因此不存在自引用借用问题。

pub mod types;

use libloading::{Library, Symbol};
use std::path::PathBuf;

use crate::error::WimError;
use types::*;

pub const DLL_NAME: &str = "libwim-15.dll";

#[allow(dead_code)]
pub struct WimlibApi {
    // 必须保持 Library 存活，DLL 才不会被卸载；放最后保证 drop 顺序无碍。
    _lib: Library,
    pub global_init: GlobalInit,
    pub global_cleanup: GlobalCleanup,
    pub free: Free,
    pub get_error_string: GetErrorString,
    pub get_version_string: GetVersionString,
    pub open_wim_with_progress: OpenWimWithProgress,
    pub verify_wim: VerifyWim,
    pub extract_image: ExtractImage,
    pub reference_resource_files: ReferenceResourceFiles,
    pub get_wim_info: GetWimInfo,
    pub get_image_property: GetImageProperty,
}

/// 在常见位置依次尝试定位 DLL：可执行文件同目录 → 当前工作目录 → 裸文件名（交给系统搜索）。
fn candidate_paths() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            v.push(dir.join(DLL_NAME));
        }
    }
    v.push(PathBuf::from(DLL_NAME));
    v
}

macro_rules! sym {
    ($lib:expr, $name:literal, $ty:ty) => {{
        let s: Symbol<$ty> = unsafe {
            $lib.get($name).map_err(|e| {
                WimError::SymbolNotFound {
                    name: $name
                        .strip_suffix(b"\0")
                        .map(|b| String::from_utf8_lossy(b).into_owned())
                        .unwrap_or_default(),
                    source: e,
                }
            })?
        };
        // 解引用拷贝出裸函数指针。
        *s
    }};
}

impl WimlibApi {
    /// 加载 DLL 并解析所有需要的符号。
    pub fn load() -> Result<Self, WimError> {
        let candidates = candidate_paths();
        let mut last_err = None;
        let mut lib = None;
        for path in &candidates {
            match unsafe { Library::new(path) } {
                Ok(l) => {
                    lib = Some(l);
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        let lib = lib.ok_or_else(|| WimError::DllNotFound {
            name: DLL_NAME.to_string(),
            inner: last_err,
        })?;

        let api = WimlibApi {
            global_init: sym!(lib, b"wimlib_global_init\0", GlobalInit),
            global_cleanup: sym!(lib, b"wimlib_global_cleanup\0", GlobalCleanup),
            free: sym!(lib, b"wimlib_free\0", Free),
            get_error_string: sym!(lib, b"wimlib_get_error_string\0", GetErrorString),
            get_version_string: sym!(lib, b"wimlib_get_version_string\0", GetVersionString),
            open_wim_with_progress: sym!(
                lib,
                b"wimlib_open_wim_with_progress\0",
                OpenWimWithProgress
            ),
            verify_wim: sym!(lib, b"wimlib_verify_wim\0", VerifyWim),
            extract_image: sym!(lib, b"wimlib_extract_image\0", ExtractImage),
            reference_resource_files: sym!(
                lib,
                b"wimlib_reference_resource_files\0",
                ReferenceResourceFiles
            ),
            get_wim_info: sym!(lib, b"wimlib_get_wim_info\0", GetWimInfo),
            get_image_property: sym!(lib, b"wimlib_get_image_property\0", GetImageProperty),
            _lib: lib,
        };

        // 全局初始化一次。
        let rc = unsafe { (api.global_init)(0) };
        if rc != WIMLIB_ERR_SUCCESS {
            return Err(WimError::from_code_with_api(rc, &api));
        }
        Ok(api)
    }

    /// 取 wimlib 版本字符串（C 字符串，ASCII）。
    pub fn version_string(&self) -> String {
        let ptr = unsafe { (self.get_version_string)() };
        if ptr.is_null() {
            return "未知".to_string();
        }
        let mut len = 0usize;
        unsafe {
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(ptr, len);
            String::from_utf8_lossy(slice).into_owned()
        }
    }
}

impl Drop for WimlibApi {
    fn drop(&mut self) {
        // 进程退出前释放 wimlib 全局资源。
        unsafe { (self.global_cleanup)() };
    }
}
