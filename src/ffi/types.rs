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

// ---- write flags（用于 convert / 制作镜像的写出） ----
pub const WIMLIB_WRITE_FLAG_CHECK_INTEGRITY: c_int = 0x0000_0001;
pub const WIMLIB_WRITE_FLAG_RECOMPRESS: c_int = 0x0000_0010;
pub const WIMLIB_WRITE_FLAG_REBUILD: c_int = 0x0000_0040;
pub const WIMLIB_WRITE_FLAG_SOLID: c_int = 0x0000_1000;

// ---- 特殊 image 索引 ----
pub const WIMLIB_ALL_IMAGES: c_int = -1;

// ---- 进度回调返回值 ----
pub const WIMLIB_PROGRESS_STATUS_CONTINUE: c_int = 0;
pub const WIMLIB_PROGRESS_STATUS_ABORT: c_int = 1;

// ---- 进度消息类型（仅列出用到的） ----
pub const WIMLIB_PROGRESS_MSG_EXTRACT_IMAGE_BEGIN: c_int = 0;
pub const WIMLIB_PROGRESS_MSG_EXTRACT_FILE_STRUCTURE: c_int = 3;
pub const WIMLIB_PROGRESS_MSG_EXTRACT_STREAMS: c_int = 4;
pub const WIMLIB_PROGRESS_MSG_WRITE_STREAMS: c_int = 12;
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

/// union wimlib_progress_info 的 `write_streams` 分支（wimlib.h:831）。
/// 仅定义到进度所需字段为止（completed_bytes 是第 3 个字段）。
#[repr(C)]
pub struct ProgressInfoWriteStreams {
    pub total_bytes: u64,
    pub total_streams: u64,
    pub completed_bytes: u64,
    pub completed_streams: u64,
    pub num_threads: u32,
    pub compression_type: i32,
    pub total_parts: u32,
    pub completed_parts: u32,
    pub completed_compressed_bytes: u64,
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
// wimlib_get_version_string 返回 `const wimlib_tchar *`，在 Windows 上即
// wchar_t（2 字节 UTF-16LE），因此是 *const u16 而非 *const u8。
pub type GetVersionString = unsafe extern "C" fn() -> *const u16;
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

// ============================================================================
// 其余导出函数的完整 FFI 绑定。第一阶段不在安全层使用，但按“DLL 导出多少就
// 绑定多少”全部声明并在 WimlibApi 中加载。复杂入参（capture source / update
// command / 压缩器句柄 / 迭代回调结构）按 wimlib.h 用不透明指针精确占位，留待
// 实现对应功能时再展开字段；const wimlib_tchar* 一律为 *const u16（UTF-16）。
// ============================================================================

// ---- 不透明结构占位（字段暂不展开，仅用于类型安全的指针传递） ----
/// struct wimlib_capture_source（wimlib.h:1285）：add_image_multisource 的输入数组元素。
#[repr(C)]
pub struct WimlibCaptureSource {
    _private: [u8; 0],
}
/// struct wimlib_update_command（wimlib.h:2520）：update_image 的命令数组元素。
#[repr(C)]
pub struct WimlibUpdateCommand {
    _private: [u8; 0],
}
/// struct wimlib_dir_entry（wimlib.h:1546）：iterate_dir_tree 回调收到的目录项。
#[repr(C)]
pub struct WimlibDirEntry {
    _private: [u8; 0],
}
/// struct wimlib_resource_entry（wimlib.h:1444）：iterate_lookup_table 回调收到的资源项。
#[repr(C)]
pub struct WimlibResourceEntry {
    _private: [u8; 0],
}
/// 不透明压缩器/解压器句柄。
#[repr(C)]
pub struct WimlibCompressor {
    _private: [u8; 0],
}
#[repr(C)]
pub struct WimlibDecompressor {
    _private: [u8; 0],
}

// ---- 回调函数类型 ----
/// `int (*)(const struct wimlib_dir_entry *, void *user_ctx)`
pub type IterateDirTreeCallback =
    Option<unsafe extern "C" fn(dentry: *const WimlibDirEntry, user_ctx: *mut c_void) -> c_int>;
/// `int (*)(const struct wimlib_resource_entry *, void *user_ctx)`
pub type IterateLookupTableCallback = Option<
    unsafe extern "C" fn(resource: *const WimlibResourceEntry, user_ctx: *mut c_void) -> c_int,
>;
/// set_memory_allocator 的三个分配器回调（malloc / free / realloc）。
pub type MallocFunc = Option<unsafe extern "C" fn(size: usize) -> *mut c_void>;
pub type FreeFunc = Option<unsafe extern "C" fn(ptr: *mut c_void)>;
pub type ReallocFunc = Option<unsafe extern "C" fn(ptr: *mut c_void, size: usize) -> *mut c_void>;

// ---- 镜像构建 / 修改 ----
pub type CreateNewWim = unsafe extern "C" fn(ctype: c_int, wim_ret: *mut *mut WimStruct) -> c_int;
pub type AddEmptyImage =
    unsafe extern "C" fn(wim: *mut WimStruct, name: *const u16, new_idx_ret: *mut c_int) -> c_int;
pub type AddImage = unsafe extern "C" fn(
    wim: *mut WimStruct,
    source: *const u16,
    name: *const u16,
    config_file: *const u16,
    add_flags: c_int,
) -> c_int;
pub type AddImageMultisource = unsafe extern "C" fn(
    wim: *mut WimStruct,
    sources: *const WimlibCaptureSource,
    num_sources: usize,
    name: *const u16,
    config_file: *const u16,
    add_flags: c_int,
) -> c_int;
pub type AddTree = unsafe extern "C" fn(
    wim: *mut WimStruct,
    image: c_int,
    fs_source_path: *const u16,
    wim_target_path: *const u16,
    add_flags: c_int,
) -> c_int;
pub type DeleteImage = unsafe extern "C" fn(wim: *mut WimStruct, image: c_int) -> c_int;
pub type DeletePath = unsafe extern "C" fn(
    wim: *mut WimStruct,
    image: c_int,
    path: *const u16,
    delete_flags: c_int,
) -> c_int;
pub type RenamePath = unsafe extern "C" fn(
    wim: *mut WimStruct,
    image: c_int,
    source_path: *const u16,
    dest_path: *const u16,
) -> c_int;
pub type UpdateImage = unsafe extern "C" fn(
    wim: *mut WimStruct,
    image: c_int,
    cmds: *const WimlibUpdateCommand,
    num_cmds: usize,
    update_flags: c_int,
) -> c_int;
pub type ExportImage = unsafe extern "C" fn(
    src_wim: *mut WimStruct,
    src_image: c_int,
    dest_wim: *mut WimStruct,
    dest_name: *const u16,
    dest_description: *const u16,
    export_flags: c_int,
) -> c_int;
pub type ReferenceTemplateImage = unsafe extern "C" fn(
    wim: *mut WimStruct,
    new_image: c_int,
    template_wim: *mut WimStruct,
    template_image: c_int,
    flags: c_int,
) -> c_int;

// ---- 写出 / 覆盖 / 分卷合并 ----
pub type Write = unsafe extern "C" fn(
    wim: *mut WimStruct,
    path: *const u16,
    image: c_int,
    write_flags: c_int,
    num_threads: c_uint,
) -> c_int;
pub type WriteToFd = unsafe extern "C" fn(
    wim: *mut WimStruct,
    fd: c_int,
    image: c_int,
    write_flags: c_int,
    num_threads: c_uint,
) -> c_int;
pub type Overwrite =
    unsafe extern "C" fn(wim: *mut WimStruct, write_flags: c_int, num_threads: c_uint) -> c_int;
pub type Split = unsafe extern "C" fn(
    wim: *mut WimStruct,
    swm_name: *const u16,
    part_size: u64,
    write_flags: c_int,
) -> c_int;
pub type Join = unsafe extern "C" fn(
    swms: *const *const u16,
    num_swms: c_uint,
    output_path: *const u16,
    swm_open_flags: c_int,
    wim_write_flags: c_int,
) -> c_int;
pub type JoinWithProgress = unsafe extern "C" fn(
    swms: *const *const u16,
    num_swms: c_uint,
    output_path: *const u16,
    swm_open_flags: c_int,
    wim_write_flags: c_int,
    progfunc: ProgressFunc,
    progctx: *mut c_void,
) -> c_int;

// ---- 打开（无进度变体）/ 引用 ----
pub type OpenWim =
    unsafe extern "C" fn(wim_file: *const u16, open_flags: c_int, wim_ret: *mut *mut WimStruct) -> c_int;
pub type ReferenceResources = unsafe extern "C" fn(
    wim: *mut WimStruct,
    resource_wims: *mut *mut WimStruct,
    num_resource_wims: c_uint,
    ref_flags: c_int,
) -> c_int;
pub type RegisterProgressFunction =
    unsafe extern "C" fn(wim: *mut WimStruct, progfunc: ProgressFunc, progctx: *mut c_void);

// ---- 解包扩展（路径 / 管道） ----
pub type ExtractPaths = unsafe extern "C" fn(
    wim: *mut WimStruct,
    image: c_int,
    target: *const u16,
    paths: *const *const u16,
    num_paths: usize,
    extract_flags: c_int,
) -> c_int;
pub type ExtractPathlist = unsafe extern "C" fn(
    wim: *mut WimStruct,
    image: c_int,
    target: *const u16,
    path_list_file: *const u16,
    extract_flags: c_int,
) -> c_int;
pub type ExtractImageFromPipe = unsafe extern "C" fn(
    pipe_fd: c_int,
    image_num_or_name: *const u16,
    target: *const u16,
    extract_flags: c_int,
) -> c_int;
pub type ExtractImageFromPipeWithProgress = unsafe extern "C" fn(
    pipe_fd: c_int,
    image_num_or_name: *const u16,
    target: *const u16,
    extract_flags: c_int,
    progfunc: ProgressFunc,
    progctx: *mut c_void,
) -> c_int;

// ---- 信息 / 查询 / 迭代 ----
pub type GetVersion = unsafe extern "C" fn() -> u32;
pub type GetCompressionTypeString = unsafe extern "C" fn(ctype: c_int) -> *const u16;
pub type GetImageName = unsafe extern "C" fn(wim: *const WimStruct, image: c_int) -> *const u16;
pub type GetImageDescription =
    unsafe extern "C" fn(wim: *const WimStruct, image: c_int) -> *const u16;
pub type ImageNameInUse = unsafe extern "C" fn(wim: *const WimStruct, name: *const u16) -> bool;
pub type ResolveImage =
    unsafe extern "C" fn(wim: *mut WimStruct, image_name_or_num: *const u16) -> c_int;
pub type GetXmlData = unsafe extern "C" fn(
    wim: *mut WimStruct,
    buf_ret: *mut *mut c_void,
    bufsize_ret: *mut usize,
) -> c_int;
pub type ExtractXmlData = unsafe extern "C" fn(wim: *mut WimStruct, fp: *mut c_void) -> c_int;
pub type PrintAvailableImages = unsafe extern "C" fn(wim: *const WimStruct, image: c_int);
pub type PrintHeader = unsafe extern "C" fn(wim: *const WimStruct);
pub type IterateDirTree = unsafe extern "C" fn(
    wim: *mut WimStruct,
    image: c_int,
    path: *const u16,
    flags: c_int,
    cb: IterateDirTreeCallback,
    user_ctx: *mut c_void,
) -> c_int;
pub type IterateLookupTable = unsafe extern "C" fn(
    wim: *mut WimStruct,
    flags: c_int,
    cb: IterateLookupTableCallback,
    user_ctx: *mut c_void,
) -> c_int;

// ---- 设置属性 / 元数据 / 输出参数 ----
pub type SetImageName =
    unsafe extern "C" fn(wim: *mut WimStruct, image: c_int, name: *const u16) -> c_int;
/// 注意：wimlib 头文件与导出符号此处均为历史拼写 "descripton"（少一个 i），不可写成 description。
pub type SetImageDescripton =
    unsafe extern "C" fn(wim: *mut WimStruct, image: c_int, description: *const u16) -> c_int;
pub type SetImageFlags =
    unsafe extern "C" fn(wim: *mut WimStruct, image: c_int, flags: *const u16) -> c_int;
pub type SetImageProperty = unsafe extern "C" fn(
    wim: *mut WimStruct,
    image: c_int,
    property_name: *const u16,
    property_value: *const u16,
) -> c_int;
pub type SetWimInfo =
    unsafe extern "C" fn(wim: *mut WimStruct, info: *const WimInfo, which: c_int) -> c_int;
pub type SetOutputChunkSize = unsafe extern "C" fn(wim: *mut WimStruct, chunk_size: u32) -> c_int;
pub type SetOutputPackChunkSize =
    unsafe extern "C" fn(wim: *mut WimStruct, chunk_size: u32) -> c_int;
pub type SetOutputCompressionType =
    unsafe extern "C" fn(wim: *mut WimStruct, ctype: c_int) -> c_int;
pub type SetOutputPackCompressionType =
    unsafe extern "C" fn(wim: *mut WimStruct, ctype: c_int) -> c_int;

// ---- 挂载（Windows 构建通常不支持，但符号仍导出，照样绑定） ----
pub type MountImage = unsafe extern "C" fn(
    wim: *mut WimStruct,
    image: c_int,
    dir: *const u16,
    mount_flags: c_int,
    staging_dir: *const u16,
) -> c_int;
pub type UnmountImage = unsafe extern "C" fn(dir: *const u16, unmount_flags: c_int) -> c_int;
pub type UnmountImageWithProgress = unsafe extern "C" fn(
    dir: *const u16,
    unmount_flags: c_int,
    progfunc: ProgressFunc,
    progctx: *mut c_void,
) -> c_int;

// ---- 全局设置 / 错误输出 / 文本文件 ----
pub type SetPrintErrors = unsafe extern "C" fn(show_messages: bool) -> c_int;
pub type SetErrorFile = unsafe extern "C" fn(fp: *mut c_void) -> c_int;
pub type SetErrorFileByName = unsafe extern "C" fn(path: *const u16) -> c_int;
pub type SetMemoryAllocator = unsafe extern "C" fn(
    malloc_func: MallocFunc,
    free_func: FreeFunc,
    realloc_func: ReallocFunc,
) -> c_int;
pub type LoadTextFile = unsafe extern "C" fn(
    path: *const u16,
    tstr_ret: *mut *mut u16,
    tstr_nchars_ret: *mut usize,
) -> c_int;

// ---- 独立压缩 / 解压 API ----
pub type SetDefaultCompressionLevel =
    unsafe extern "C" fn(ctype: c_int, compression_level: c_uint) -> c_int;
pub type GetCompressorNeededMemory =
    unsafe extern "C" fn(ctype: c_int, max_block_size: usize, compression_level: c_uint) -> u64;
pub type CreateCompressor = unsafe extern "C" fn(
    ctype: c_int,
    max_block_size: usize,
    compression_level: c_uint,
    compressor_ret: *mut *mut WimlibCompressor,
) -> c_int;
pub type Compress = unsafe extern "C" fn(
    uncompressed_data: *const c_void,
    uncompressed_size: usize,
    compressed_data: *mut c_void,
    compressed_size_avail: usize,
    compressor: *mut WimlibCompressor,
) -> usize;
pub type FreeCompressor = unsafe extern "C" fn(compressor: *mut WimlibCompressor);
pub type CreateDecompressor = unsafe extern "C" fn(
    ctype: c_int,
    max_block_size: usize,
    decompressor_ret: *mut *mut WimlibDecompressor,
) -> c_int;
pub type Decompress = unsafe extern "C" fn(
    compressed_data: *const c_void,
    compressed_size: usize,
    uncompressed_data: *mut c_void,
    uncompressed_size: usize,
    decompressor: *mut WimlibDecompressor,
) -> c_int;
pub type FreeDecompressor = unsafe extern "C" fn(decompressor: *mut WimlibDecompressor);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compression_names() {
        assert_eq!(compression_type_name(WIMLIB_COMPRESSION_TYPE_NONE), "无压缩");
        assert_eq!(compression_type_name(WIMLIB_COMPRESSION_TYPE_XPRESS), "XPRESS");
        assert_eq!(compression_type_name(WIMLIB_COMPRESSION_TYPE_LZX), "LZX");
        assert_eq!(
            compression_type_name(WIMLIB_COMPRESSION_TYPE_LZMS),
            "LZMS (solid)"
        );
        assert_eq!(compression_type_name(42), "未知");
    }

    #[test]
    fn integrity_flag_is_low_bit() {
        let mut info = WimInfo::zeroed();
        assert!(!info.has_integrity_table());
        info.flags = 0x1;
        assert!(info.has_integrity_table());
        // 除最低位以外的任何位都不应被误判为“含完整性表”。
        info.flags = 0xFFFF_FFFE;
        assert!(!info.has_integrity_table());
    }

    // 与 C struct wimlib_wim_info（wimlib.h:1339）的内存布局必须逐字节一致。
    // 字段顺序/类型一旦被误改，此断言会立刻失败，避免悄悄破坏 FFI 读取。
    #[test]
    fn wim_info_matches_c_layout() {
        assert_eq!(std::mem::size_of::<WimInfo>(), 88);
        assert_eq!(std::mem::align_of::<WimInfo>(), 8);
    }
}
