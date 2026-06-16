//! 运行时动态加载 libwim-15.dll，解析并缓存第一阶段需要的函数指针。
//!
//! 设计：把 `Library` 与从中解析出的裸函数指针一起存放在 `WimlibApi` 中。
//! 函数指针只是地址（Copy），只要 `Library` 不被 drop（即 DLL 保持加载），
//! 这些地址就始终有效，因此不存在自引用借用问题。

pub mod types;

use libloading::{Library, Symbol};
use std::path::PathBuf;

use crate::error::WimError;
use crate::util::from_wide;
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

    // —— 完整 FFI 绑定：DLL 共导出 72 个函数，此处补齐其余 61 个。第一阶段不在
    //    安全层调用，字段“仅写不读”由 struct 上的 #[allow(dead_code)] 容许。 ——
    pub create_new_wim: CreateNewWim,
    pub add_empty_image: AddEmptyImage,
    pub add_image: AddImage,
    pub add_image_multisource: AddImageMultisource,
    pub add_tree: AddTree,
    pub delete_image: DeleteImage,
    pub delete_path: DeletePath,
    pub rename_path: RenamePath,
    pub update_image: UpdateImage,
    pub export_image: ExportImage,
    pub reference_template_image: ReferenceTemplateImage,
    pub write: Write,
    pub write_to_fd: WriteToFd,
    pub overwrite: Overwrite,
    pub split: Split,
    pub join: Join,
    pub join_with_progress: JoinWithProgress,
    pub open_wim: OpenWim,
    pub reference_resources: ReferenceResources,
    pub register_progress_function: RegisterProgressFunction,
    pub extract_paths: ExtractPaths,
    pub extract_pathlist: ExtractPathlist,
    pub extract_image_from_pipe: ExtractImageFromPipe,
    pub extract_image_from_pipe_with_progress: ExtractImageFromPipeWithProgress,
    pub get_version: GetVersion,
    pub get_compression_type_string: GetCompressionTypeString,
    pub get_image_name: GetImageName,
    pub get_image_description: GetImageDescription,
    pub image_name_in_use: ImageNameInUse,
    pub resolve_image: ResolveImage,
    pub get_xml_data: GetXmlData,
    pub extract_xml_data: ExtractXmlData,
    pub print_available_images: PrintAvailableImages,
    pub print_header: PrintHeader,
    pub iterate_dir_tree: IterateDirTree,
    pub iterate_lookup_table: IterateLookupTable,
    pub set_image_name: SetImageName,
    pub set_image_descripton: SetImageDescripton,
    pub set_image_flags: SetImageFlags,
    pub set_image_property: SetImageProperty,
    pub set_wim_info: SetWimInfo,
    pub set_output_chunk_size: SetOutputChunkSize,
    pub set_output_pack_chunk_size: SetOutputPackChunkSize,
    pub set_output_compression_type: SetOutputCompressionType,
    pub set_output_pack_compression_type: SetOutputPackCompressionType,
    pub mount_image: MountImage,
    pub unmount_image: UnmountImage,
    pub unmount_image_with_progress: UnmountImageWithProgress,
    pub set_print_errors: SetPrintErrors,
    pub set_error_file: SetErrorFile,
    pub set_error_file_by_name: SetErrorFileByName,
    pub set_memory_allocator: SetMemoryAllocator,
    pub load_text_file: LoadTextFile,
    pub set_default_compression_level: SetDefaultCompressionLevel,
    pub get_compressor_needed_memory: GetCompressorNeededMemory,
    pub create_compressor: CreateCompressor,
    pub compress: Compress,
    pub free_compressor: FreeCompressor,
    pub create_decompressor: CreateDecompressor,
    pub decompress: Decompress,
    pub free_decompressor: FreeDecompressor,
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

            create_new_wim: sym!(lib, b"wimlib_create_new_wim\0", CreateNewWim),
            add_empty_image: sym!(lib, b"wimlib_add_empty_image\0", AddEmptyImage),
            add_image: sym!(lib, b"wimlib_add_image\0", AddImage),
            add_image_multisource: sym!(
                lib,
                b"wimlib_add_image_multisource\0",
                AddImageMultisource
            ),
            add_tree: sym!(lib, b"wimlib_add_tree\0", AddTree),
            delete_image: sym!(lib, b"wimlib_delete_image\0", DeleteImage),
            delete_path: sym!(lib, b"wimlib_delete_path\0", DeletePath),
            rename_path: sym!(lib, b"wimlib_rename_path\0", RenamePath),
            update_image: sym!(lib, b"wimlib_update_image\0", UpdateImage),
            export_image: sym!(lib, b"wimlib_export_image\0", ExportImage),
            reference_template_image: sym!(
                lib,
                b"wimlib_reference_template_image\0",
                ReferenceTemplateImage
            ),
            write: sym!(lib, b"wimlib_write\0", Write),
            write_to_fd: sym!(lib, b"wimlib_write_to_fd\0", WriteToFd),
            overwrite: sym!(lib, b"wimlib_overwrite\0", Overwrite),
            split: sym!(lib, b"wimlib_split\0", Split),
            join: sym!(lib, b"wimlib_join\0", Join),
            join_with_progress: sym!(lib, b"wimlib_join_with_progress\0", JoinWithProgress),
            open_wim: sym!(lib, b"wimlib_open_wim\0", OpenWim),
            reference_resources: sym!(lib, b"wimlib_reference_resources\0", ReferenceResources),
            register_progress_function: sym!(
                lib,
                b"wimlib_register_progress_function\0",
                RegisterProgressFunction
            ),
            extract_paths: sym!(lib, b"wimlib_extract_paths\0", ExtractPaths),
            extract_pathlist: sym!(lib, b"wimlib_extract_pathlist\0", ExtractPathlist),
            extract_image_from_pipe: sym!(
                lib,
                b"wimlib_extract_image_from_pipe\0",
                ExtractImageFromPipe
            ),
            extract_image_from_pipe_with_progress: sym!(
                lib,
                b"wimlib_extract_image_from_pipe_with_progress\0",
                ExtractImageFromPipeWithProgress
            ),
            get_version: sym!(lib, b"wimlib_get_version\0", GetVersion),
            get_compression_type_string: sym!(
                lib,
                b"wimlib_get_compression_type_string\0",
                GetCompressionTypeString
            ),
            get_image_name: sym!(lib, b"wimlib_get_image_name\0", GetImageName),
            get_image_description: sym!(
                lib,
                b"wimlib_get_image_description\0",
                GetImageDescription
            ),
            image_name_in_use: sym!(lib, b"wimlib_image_name_in_use\0", ImageNameInUse),
            resolve_image: sym!(lib, b"wimlib_resolve_image\0", ResolveImage),
            get_xml_data: sym!(lib, b"wimlib_get_xml_data\0", GetXmlData),
            extract_xml_data: sym!(lib, b"wimlib_extract_xml_data\0", ExtractXmlData),
            print_available_images: sym!(
                lib,
                b"wimlib_print_available_images\0",
                PrintAvailableImages
            ),
            print_header: sym!(lib, b"wimlib_print_header\0", PrintHeader),
            iterate_dir_tree: sym!(lib, b"wimlib_iterate_dir_tree\0", IterateDirTree),
            iterate_lookup_table: sym!(
                lib,
                b"wimlib_iterate_lookup_table\0",
                IterateLookupTable
            ),
            set_image_name: sym!(lib, b"wimlib_set_image_name\0", SetImageName),
            // 历史拼写 typo：DLL 导出名确实是 "descripton"（少一个 i）。
            set_image_descripton: sym!(
                lib,
                b"wimlib_set_image_descripton\0",
                SetImageDescripton
            ),
            set_image_flags: sym!(lib, b"wimlib_set_image_flags\0", SetImageFlags),
            set_image_property: sym!(lib, b"wimlib_set_image_property\0", SetImageProperty),
            set_wim_info: sym!(lib, b"wimlib_set_wim_info\0", SetWimInfo),
            set_output_chunk_size: sym!(
                lib,
                b"wimlib_set_output_chunk_size\0",
                SetOutputChunkSize
            ),
            set_output_pack_chunk_size: sym!(
                lib,
                b"wimlib_set_output_pack_chunk_size\0",
                SetOutputPackChunkSize
            ),
            set_output_compression_type: sym!(
                lib,
                b"wimlib_set_output_compression_type\0",
                SetOutputCompressionType
            ),
            set_output_pack_compression_type: sym!(
                lib,
                b"wimlib_set_output_pack_compression_type\0",
                SetOutputPackCompressionType
            ),
            mount_image: sym!(lib, b"wimlib_mount_image\0", MountImage),
            unmount_image: sym!(lib, b"wimlib_unmount_image\0", UnmountImage),
            unmount_image_with_progress: sym!(
                lib,
                b"wimlib_unmount_image_with_progress\0",
                UnmountImageWithProgress
            ),
            set_print_errors: sym!(lib, b"wimlib_set_print_errors\0", SetPrintErrors),
            set_error_file: sym!(lib, b"wimlib_set_error_file\0", SetErrorFile),
            set_error_file_by_name: sym!(
                lib,
                b"wimlib_set_error_file_by_name\0",
                SetErrorFileByName
            ),
            set_memory_allocator: sym!(
                lib,
                b"wimlib_set_memory_allocator\0",
                SetMemoryAllocator
            ),
            load_text_file: sym!(lib, b"wimlib_load_text_file\0", LoadTextFile),
            set_default_compression_level: sym!(
                lib,
                b"wimlib_set_default_compression_level\0",
                SetDefaultCompressionLevel
            ),
            get_compressor_needed_memory: sym!(
                lib,
                b"wimlib_get_compressor_needed_memory\0",
                GetCompressorNeededMemory
            ),
            create_compressor: sym!(lib, b"wimlib_create_compressor\0", CreateCompressor),
            compress: sym!(lib, b"wimlib_compress\0", Compress),
            free_compressor: sym!(lib, b"wimlib_free_compressor\0", FreeCompressor),
            create_decompressor: sym!(
                lib,
                b"wimlib_create_decompressor\0",
                CreateDecompressor
            ),
            decompress: sym!(lib, b"wimlib_decompress\0", Decompress),
            free_decompressor: sym!(lib, b"wimlib_free_decompressor\0", FreeDecompressor),

            _lib: lib,
        };

        // 全局初始化一次。
        let rc = unsafe { (api.global_init)(0) };
        if rc != WIMLIB_ERR_SUCCESS {
            return Err(WimError::from_code_with_api(rc, &api));
        }
        Ok(api)
    }

    /// 取 wimlib 版本字符串（例如 "1.14.4"）。
    ///
    /// `wimlib_get_version_string` 返回 `const wimlib_tchar *`——在 Windows 上即
    /// UTF-16（`wchar_t`），指向库内静态分配的字符串，读完即用、无需释放。
    /// 复用统一的 `from_wide` 解码，不在此重复手写宽字符扫描。
    pub fn version_string(&self) -> String {
        let ptr = unsafe { (self.get_version_string)() };
        unsafe { from_wide(ptr) }.unwrap_or_else(|| "未知".to_string())
    }
}

impl Drop for WimlibApi {
    fn drop(&mut self) {
        // 进程退出前释放 wimlib 全局资源。
        unsafe { (self.global_cleanup)() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 冒烟测试：成功加载即意味着全部 72 个导出符号都已解析——任一拼错或缺失都会
    // 让 load() 返回 SymbolNotFound。顺带校验 version_string 修复后能正确解码
    // UTF-16（不再被旧的 UTF-8 读法截断成 "1"）。仅在 Windows + DLL 就位时运行。
    #[test]
    fn load_resolves_all_symbols_and_decodes_version() {
        let api = WimlibApi::load().expect("加载 libwim-15.dll 并解析全部 72 个符号应成功");

        let v = api.version_string();
        assert!(v.contains('.'), "版本号应形如 x.y.z，实际: {v:?}");
        assert_ne!(v, "1", "version_string 不应被截断成 \"1\"（旧 UTF-8 解码 bug）");

        // 已绑定的 wimlib_get_version 应返回打包的非 0 版本号。
        let packed = unsafe { (api.get_version)() };
        assert!(packed > 0, "get_version 应返回非 0 版本号");
    }
}
