# Rust 镜像工具 — wimlib 封装 + CLI（Claude Code 开发规格）

## 0. 你的任务
用 Rust 封装 wimlib（通过运行时加载 `libwim-15.dll`），实现一个命令行镜像工具。
**第一阶段只做三件事，做扎实再扩展：**
1. **镜像校验**（verify integrity）—— 带实时进度 + 速度 + ETA
2. **解包/应用镜像**（apply image）支持 **WIM / ESD / SWM** 三种格式 —— 带实时进度 + 速度 + ETA
3. **读取镜像信息**（列出有几个卷、卷名、版本、压缩方式、各卷大小）

后续阶段（先不要写）：制作镜像、ESD↔WIM 转换、分卷/合并。架构要给这些留好扩展位。

## 1. 已确定的技术决策（不要改）
- 语言：Rust（edition 2021）
- 目标平台：**Windows x86_64**（`x86_64-pc-windows-msvc`）
- wimlib 接入方式：**运行时动态加载**，用 `libloading` crate 加载程序目录下的 `libwim-15.dll`。
  - 不要用编译期静态链接 import lib。这样 DLL 缺失时能给用户友好提示，而不是程序直接崩。
- DLL 放在 **可执行文件同目录**。`vendor/libwim-15.dll` 是官方 1.14.4 自包含构建，**仅依赖 5 个 Windows 系统 DLL，无第三方依赖、无需 VC++ 运行库**。构建时复制到输出目录。
- 进度展示：CLI 用 `indicatif` 画进度条（百分比 + 速度 + ETA）。
- 错误处理：`thiserror` 定义错误类型，把 wimlib 错误码映射进去。

## 2. ⚠️ 三个必须遵守的 FFI 约束（最容易翻车）
1. **路径是 UTF-16 宽字符。** 头文件里 `wimlib_tchar` 在 Windows = `wchar_t`（2 字节 UTF-16LE）。
   所有接收路径的函数（open / extract / reference 等）必须传 `*const u16`（UTF-16，NUL 结尾）。
   写一个 helper：`fn to_wide(s: &str) -> Vec<u16>`（用 `OsStr::encode_wide` + 追加 0）。
   **绝对不能**把 Rust 的 UTF-8 `&str`/`CString` 直接传进去。
2. **调用约定是 cdecl。** 所有函数指针声明为 `extern "C"`。
3. **生命周期**：`wimlib_global_init()` 全程调一次；每个打开的 `WIMStruct*` 用完必须 `wimlib_free()`；
   从 wimlib 返回的字符串/结构指针在 `wimlib_free` 后失效，读完即用。

## 3. 进度 + 速度的实现机制
wimlib 通过回调函数推送进度。回调签名（C）：
```c
enum wimlib_progress_status (*)(enum wimlib_progress_msg msg_type,
                                union wimlib_progress_info *info,
                                void *progctx);
```
- 返回 `WIMLIB_PROGRESS_STATUS_CONTINUE`(0) 继续，`WIMLIB_PROGRESS_STATUS_ABORT`(1) 中止（用于"取消"）。
- `progctx` 是你传入的上下文指针 —— 把一个 Rust 结构体（含进度条句柄、上次字节数、起始时间）的指针传进去，在回调里取回。
- 在 Rust 侧把回调写成 `extern "C" fn`，**回调里务必 `catch_unwind`**，不能让 panic 跨越 FFI 边界。

**关键消息和字段：**
- 解包：`WIMLIB_PROGRESS_MSG_EXTRACT_STREAMS` → 读 `info.extract.completed_bytes` / `info.extract.total_bytes`（这是进度+速度的主数据源）。
  另有 `EXTRACT_IMAGE_BEGIN` / `EXTRACT_FILE_STRUCTURE` / `EXTRACT_METADATA` / `EXTRACT_IMAGE_END` 用于阶段提示。
- 校验：`WIMLIB_PROGRESS_MSG_VERIFY_INTEGRITY` → 读 `info.integrity.completed_bytes` / `info.integrity.total_bytes`。

**`union wimlib_progress_info` 处理建议**：这个 union 有 ~20 个分支、很大。第一阶段**不要**完整翻译。
只为用到的两个分支定义 `#[repr(C)]` 结构（`extract` 和 `integrity` 的前若干字段，按 wimlib.h 偏移对齐），
在回调里根据 `msg_type` 把 `info` 指针 `cast` 成对应结构读取。`wimlib.h` 在仓库 `vendor/wimlib.h`，按它的字段顺序定义。

**速度/ETA 自己算**（wimlib 只给字节数）：在 progctx 里存 `Instant` 起点和上次采样，
速度 = Δbytes/Δt（建议做个 1~2 秒窗口的平滑），ETA = (total-completed)/speed。`indicatif` 也能直接显示。

## 4. 各功能用到的 wimlib 函数（这些名字已和 vendor/wimlib.h 核对过）
- 初始化：`wimlib_global_init(0)`
- 打开：`wimlib_open_wim_with_progress(path_w, open_flags, &mut wim, cb, ctx)`
  - 校验时 open_flags 传 `WIMLIB_OPEN_FLAG_CHECK_INTEGRITY`，会在打开时触发 VERIFY_INTEGRITY 进度。
- 校验：`wimlib_verify_wim(wim, 0)`（或仅靠上面的 CHECK_INTEGRITY 打开）。无完整性表的镜像会跳过哈希校验，需向用户说明。
- 解包整卷：`wimlib_extract_image(wim, image_index, target_dir_w, extract_flags)`
  - `image_index` 从 1 开始；解全部用 `WIMLIB_ALL_IMAGES`。普通解到目录 extract_flags 传 0。
- **SWM 分卷**：先 `wimlib_open_wim_with_progress` 打开第一片（如 `test.swm`），
  再 `wimlib_reference_resource_files(wim, &["路径/test*.swm"], 1, WIMLIB_REF_FLAG_GLOB_ENABLE, 0)` 引入其余分卷，然后照常 extract。
- 信息：`wimlib_get_wim_info(wim, &mut info)` 拿 `image_count` / `total_parts` / `compression_type` / `chunk_size` 等；
  逐卷 `wimlib_get_image_property(wim, i, prop_w)` 读 `"NAME"` `"DESCRIPTION"` `"DISPLAYNAME"` `"FLAGS"` `"WINDOWS/PRODUCTNAME"` 等。
- 错误信息：`wimlib_get_error_string(code)`（返回宽字符，转回 String 给用户）。
- 释放：`wimlib_free(wim)`。
- **ESD 无需特殊处理**：它就是 solid 压缩的 WIM，上面的 open/extract/info 全部通用。

## 5. 建议的项目结构
```
src/
  ffi/
    mod.rs        // libloading 加载 DLL，缓存函数指针（一个 struct WimlibApi）
    types.rs      // #[repr(C)] 的 wim_info / progress_info 分支 / 枚举常量
    callback.rs   // extern "C" 进度回调 + catch_unwind + progctx 结构
  wim.rs          // 安全封装层：Wim 句柄(RAII Drop=free)、open/verify/extract/info
  error.rs        // WimError + 错误码映射
  progress.rs     // 速度/ETA 计算 + indicatif 进度条封装
  cli.rs          // clap 子命令: verify / extract / info
  main.rs
vendor/           // libwim-15.dll, libwim.lib, wimlib.h
fixtures/         // 测试夹具（见下）
build.rs          // 把 vendor/libwim-15.dll 复制到 target 输出目录
```
CLI 设计（用 clap）：
- `imgtool info <镜像>` → 打印卷数、各卷名/版本/大小、压缩方式
- `imgtool verify <镜像>` → 进度条 + 结果（OK / 损坏，退出码区分）
- `imgtool extract <镜像> --index <N|all> --dest <目录>` → 进度条
  - SWM 传第一片路径即可，自动 glob 同目录其余分卷

## 6. 测试（重要：先用夹具，别一上来下 5GB）
`fixtures/` 里是用 wimlib 1.14.4 生成的小夹具，和 vendor 里的 DLL 同版本、格式兼容：
- `test.wim` —— 2 个 image（ProEdition / HomeEdition），LZX 压缩，**含完整性表**
- `test.esd` —— 同内容，solid/LZMS 压缩（验证 ESD 路径）
- `test.swm` + `test2.swm` + `test3.swm` —— 3 片分卷（验证 SWM 引用+解包）
- `corrupt.wim` —— 故意损坏，**校验必须失败**（wimlib 返回错误码 13）

**每个功能的验收用例：**
- `info test.wim` → 报告 image_count=2，列出两个卷名
- `verify test.wim` → 成功；`verify corrupt.wim` → 报告损坏且退出码非 0
- `extract test.wim --index 1 --dest out/` → 解出文件，与原始内容一致（可比对 readme.txt）
- `extract test.esd ...` → 同样成功（证明 ESD 通路）
- `extract test.swm --dest out/` → 自动合并 3 片并解出
- 进度回调：解包/校验时进度条从 0% 到 100%，速度和 ETA 有合理数值

跑通夹具后，用仓库 `test_links.txt` 里的**微软官方 ESD**（最小的 Win10 1909，3.36GB）做一次真实大文件测试，重点看：长时间运行稳定、进度/速度/ETA 准确、ESD 多卷信息正确解析。

## 7. 开发顺序（建议）
1. `ffi/mod.rs`：libloading 加载 DLL + 解析全部要用的函数指针；写个冒烟测试调 `wimlib_global_init` 成功。
2. `error.rs` + `wim.rs` 的 open/free/get_wim_info；`info` 子命令跑通（先不带进度）。
3. 进度回调 + `progress.rs`；`verify` 子命令跑通（含 corrupt 失败用例）。
4. `extract` 子命令（WIM）跑通。
5. SWM 引用、ESD 验证。
6. 真实链接大文件测试。

## 8. 注意事项
- 回调里 panic 必须 `catch_unwind` 拦下，转成 ABORT 返回。
- 宽字符转换集中在一个 helper，别散落各处。
- DLL 找不到时给明确提示："未找到 libwim-15.dll，请确认它与程序在同一目录"。
- 所有从 wimlib 拿到的指针，使用前判空。
