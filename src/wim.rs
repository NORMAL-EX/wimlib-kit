//! 安全封装层：RAII 的 `Wim` 句柄（Drop 时 wimlib_free），以及 open/info/verify/extract。

use std::os::raw::{c_int, c_void};
use std::ptr;

use crate::callback::progress_callback;
use crate::error::WimError;
use crate::ffi::types::*;
use crate::ffi::WimlibApi;
use crate::util::{from_wide, to_wide};

/// 一个已打开的 WIM/ESD/SWM 句柄。Drop 时自动调用 wimlib_free。
pub struct Wim<'a> {
    api: &'a WimlibApi,
    ptr: *mut WimStruct,
}

impl<'a> Wim<'a> {
    /// 打开镜像。`progctx` 非空时安装进度回调（用于 verify/extract）。
    ///
    /// # Safety 约定
    /// 调用者需保证 `progctx` 指向的对象在本 `Wim` 的整个使用期间保持有效，
    /// 因为 wimlib 会把该指针存进 WIMStruct，供后续操作回调使用。
    pub fn open(
        api: &'a WimlibApi,
        path: &str,
        open_flags: c_int,
        progctx: *mut c_void,
    ) -> Result<Self, WimError> {
        let wpath = to_wide(path);
        let mut ptr: *mut WimStruct = ptr::null_mut();
        let progfunc: ProgressFunc = if progctx.is_null() {
            None
        } else {
            Some(progress_callback as unsafe extern "C" fn(c_int, *mut c_void, *mut c_void) -> c_int)
        };

        let rc = unsafe {
            (api.open_wim_with_progress)(wpath.as_ptr(), open_flags, &mut ptr, progfunc, progctx)
        };
        if rc != WIMLIB_ERR_SUCCESS {
            return Err(WimError::from_code_with_api(rc, api));
        }
        if ptr.is_null() {
            return Err(WimError::Other(
                "wimlib_open_wim 返回成功但句柄为空".to_string(),
            ));
        }
        Ok(Wim { api, ptr })
    }

    /// 读取基础信息（卷数、版本、压缩方式、总字节、是否含完整性表等）。
    pub fn info(&self) -> Result<WimInfo, WimError> {
        let mut info = WimInfo::zeroed();
        let rc = unsafe { (self.api.get_wim_info)(self.ptr, &mut info) };
        if rc != WIMLIB_ERR_SUCCESS {
            return Err(WimError::from_code_with_api(rc, self.api));
        }
        Ok(info)
    }

    /// 读取某卷的属性（如 "NAME" / "DESCRIPTION" / "WINDOWS/PRODUCTNAME"）。
    /// 返回 None 表示该属性不存在。
    pub fn image_property(&self, image: u32, prop: &str) -> Option<String> {
        let wprop = to_wide(prop);
        unsafe {
            let p = (self.api.get_image_property)(self.ptr, image as c_int, wprop.as_ptr());
            from_wide(p)
        }
    }

    /// 显式校验完整性（在已用 CHECK_INTEGRITY 打开的基础上一般无需再调）。
    #[allow(dead_code)]
    pub fn verify(&self) -> Result<(), WimError> {
        let rc = unsafe { (self.api.verify_wim)(self.ptr, 0) };
        if rc != WIMLIB_ERR_SUCCESS {
            return Err(WimError::from_code_with_api(rc, self.api));
        }
        Ok(())
    }

    /// 为 SWM 分卷引入其余分片（glob 形如 "dir/test*.swm"）。
    pub fn reference_glob(&self, glob: &str) -> Result<(), WimError> {
        let wglob = to_wide(glob);
        let globs: [*const u16; 1] = [wglob.as_ptr()];
        let rc = unsafe {
            (self.api.reference_resource_files)(
                self.ptr,
                globs.as_ptr(),
                1,
                WIMLIB_REF_FLAG_GLOB_ENABLE,
                0,
            )
        };
        if rc != WIMLIB_ERR_SUCCESS {
            return Err(WimError::from_code_with_api(rc, self.api));
        }
        Ok(())
    }

    /// 解包指定卷到目标目录。`image` 为 1 起始的卷号，或 WIMLIB_ALL_IMAGES。
    pub fn extract(&self, image: c_int, target: &str) -> Result<(), WimError> {
        let wtarget = to_wide(target);
        let rc =
            unsafe { (self.api.extract_image)(self.ptr, image, wtarget.as_ptr(), 0) };
        if rc != WIMLIB_ERR_SUCCESS {
            return Err(WimError::from_code_with_api(rc, self.api));
        }
        Ok(())
    }
}

impl<'a> Drop for Wim<'a> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { (self.api.free)(self.ptr) };
            self.ptr = ptr::null_mut();
        }
    }
}
