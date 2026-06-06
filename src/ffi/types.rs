//! 与 wimlib.h 对齐的 #[repr(C)] 结构、枚举常量与函数指针类型。
//!
//! 仅翻译第一阶段（info / verify / extract）需要的部分。所有结构的字段顺序、
//! 类型宽度均严格按照 vendor/wimlib.h，切勿随意调整以免破坏内存布局。

use std::os::raw::{c_int, c_uint, c_void};

/// 不透明的 WIMStruct 句柄。
#[repr(C)]
pub struct WimStruct {
    _private: [u8; 0],
}

pub const WIMLIB_GUID_LEN: usize = 16;

// ---- open flags ----
pub const WIMLIB_OPEN_FLAG_CHECK_INTEGRITY: c_int = 0x0000_0001;
#[allow(dead_code)]
pub const WIMLIB_OPEN_FLAG_ERROR_IF_SPLIT: c_int = 0x0000_0002;

// ---- reference flags ----
pub const WIMLIB_REF_FLAG_GLOB_ENABLE: c_int = 0x0000_0001;

// ---- 特殊 image 索引 ----
pub const WIMLIB_ALL_IMAGES: c_int = -1;

// ---- 进度回调返回值 ----
pub const WIMLIB_PROGRESS_STATUS_CONTINUE: c_int = 0;
pub const WIMLIB_PROGRESS_STATUS_ABORT: c_int = 1;

// ---- 进度消息类型（仅列出用到的） ----
pub const WIMLIB_PROGRESS_MSG_EXTRACT_IMAGE_BEGIN: c_int = 0;
pub const WIMLIB_PROGRESS_MSG_EXTRACT_FILE_STRUCTURE: c_int = 3;
pub const WIMLIB_PROGRESS_MSG_EXTRACT_STREAMS: c_int = 4;
pub const WIMLIB_PROGRESS_MSG_EXTRACT_SPWM_PART_BEGIN: c_int = 5;
pub const WIMLIB_PROGRESS_MSG_EXTRACT_METADATA: c_int = 6;
pub const WIMLIB_PROGRESS_MSG_EXTRACT_IMAGE_END: c_int = 7;
pub const WIMLIB_PROGRESS_MSG_VERIFY_INTEGRITY: c_int = 16;

// ---- 压缩类型 ----
pub const WIMLIB_COMPRESSION_TYPE_NONE: i32 = 0;
pub const WIMLIB_COMPRESSION_TYPE_XPRESS: i32 = 1;
pub const WIMLIB_COMPRESSION_TYPE_LZX: i32 = 2;
pub const WIMLIB_COMPRESSION_TYPE_LZMS: i32 = 3;

// ---- 错误码（仅列出会显式判断的，其余走通用映射） ----
pub const WIMLIB_ERR_SUCCESS: c_int = 0;
pub const WIMLIB_ERR_INTEGRITY: c_int = 13;

/// 对应 struct wimlib_wim_info（vendor/wimlib.h:1339）。
///
/// 位域区域（has_integrity_table:1 ... reserved_flags:22）合计 32 位，
/// 这里用一个 u32 `flags` 表示；最低位即 has_integrity_table。
#[repr(C)]
pub struct WimInfo {
    pub guid: [u8; WIMLIB_GUID_LEN],
    pub image_count: u32,
    pub boot_index: u32,
    pub wim_version: u32,
    pub chunk_size: u32,
    pub part_number: u16,
    pub total_parts: u16,
    pub compression_type: i32,
    pub total_bytes: u64,
    pub flags: u32,
    pub reserved: [u32; 9],
}

impl WimInfo {
    pub fn zeroed() -> Self {
        // 全零是合法的初始状态，wimlib_get_wim_info 会填充。
        unsafe { std::mem::zeroed() }
    }

    pub fn has_integrity_table(&self) -> bool {
        self.flags & 0x1 != 0
    }
}

/// union wimlib_progress_info 的 `extract` 分支（vendor/wimlib.h:972）。
/// 仅定义到 completed_bytes 为止够用（之后的字段第一阶段不读）。
#[repr(C)]
pub struct ProgressInfoExtract {
    pub image: u32,
    pub extract_flags: u32,
    pub wimfile_name: *const u16,
    pub image_name: *const u16,
    pub target: *const u16,
    pub reserved: *const u16,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub total_streams: u64,
    pub completed_streams: u64,
    pub part_number: u32,
    pub total_parts: u32,
    pub guid: [u8; WIMLIB_GUID_LEN],
    pub current_file_count: u64,
    pub end_file_count: u64,
}

/// union wimlib_progress_info 的 `integrity` 分支（vendor/wimlib.h:1080）。
#[repr(C)]
pub struct ProgressInfoIntegrity {
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub total_chunks: u32,
    pub completed_chunks: u32,
    pub chunk_size: u32,
    pub filename: *const u16,
}

/// C 进度回调签名：
/// enum wimlib_progress_status (*)(enum wimlib_progress_msg, union wimlib_progress_info*, void*)
pub type ProgressFunc =
    Option<unsafe extern "C" fn(msg_type: c_int, info: *mut c_void, progctx: *mut c_void) -> c_int>;

// ---- 函数指针类型 ----
pub type GlobalInit = unsafe extern "C" fn(init_flags: c_int) -> c_int;
pub type GlobalCleanup = unsafe extern "C" fn();
pub type Free = unsafe extern "C" fn(wim: *mut WimStruct);
pub type GetErrorString = unsafe extern "C" fn(code: c_int) -> *const u16;
pub type GetVersionString = unsafe extern "C" fn() -> *const u8;
pub type OpenWimWithProgress = unsafe extern "C" fn(
    wim_file: *const u16,
    open_flags: c_int,
    wim_ret: *mut *mut WimStruct,
    progfunc: ProgressFunc,
    progctx: *mut c_void,
) -> c_int;
pub type VerifyWim = unsafe extern "C" fn(wim: *mut WimStruct, verify_flags: c_int) -> c_int;
pub type ExtractImage = unsafe extern "C" fn(
    wim: *mut WimStruct,
    image: c_int,
    target: *const u16,
    extract_flags: c_int,
) -> c_int;
pub type ReferenceResourceFiles = unsafe extern "C" fn(
    wim: *mut WimStruct,
    resource_wimfiles_or_globs: *const *const u16,
    count: c_uint,
    ref_flags: c_int,
    open_flags: c_int,
) -> c_int;
pub type GetWimInfo =
    unsafe extern "C" fn(wim: *mut WimStruct, info: *mut WimInfo) -> c_int;
pub type GetImageProperty = unsafe extern "C" fn(
    wim: *const WimStruct,
    image: c_int,
    property_name: *const u16,
) -> *const u16;

/// 把压缩类型常量转成可读名称。
pub fn compression_type_name(ct: i32) -> &'static str {
    match ct {
        WIMLIB_COMPRESSION_TYPE_NONE => "无压缩",
        WIMLIB_COMPRESSION_TYPE_XPRESS => "XPRESS",
        WIMLIB_COMPRESSION_TYPE_LZX => "LZX",
        WIMLIB_COMPRESSION_TYPE_LZMS => "LZMS (solid)",
        _ => "未知",
    }
}
