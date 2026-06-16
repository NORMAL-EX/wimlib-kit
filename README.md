# wimlib-kit

> 基于 [wimlib](https://wimlib.net/) 的 Windows 镜像工具：校验、解包、读取 WIM / ESD / SWM 镜像。

`wimlib-kit` 用 Rust 编写，通过**运行时动态加载** `libwim-15.dll` 封装 wimlib，提供镜像完整性校验、解包/应用、信息读取三大功能，全程带实时进度条（百分比 / 速度 / ETA）。编译产出的命令行程序名为 `imgtool`。

## 特性

- **三种格式通吃**：WIM、ESD（solid/LZMS 压缩的 WIM）、SWM（分卷，自动合并）。
- **校验** `verify`：基于完整性表逐块校验，损坏时以专门退出码报错。
- **解包/应用** `extract`：整卷解出到目录，支持单卷或全部卷。
- **格式转换** `convert`：ESD↔WIM 互转与重压缩（LZX / LZMS / XPRESS），把全部卷导出到新容器。
- **制作镜像** `capture`：把文件系统目录捕获打包成 WIM/ESD。
- **分卷 / 合并** `split` / `join`：把大镜像切成 SWM 多片（FAT32 友好），或合并回 WIM。
- **信息读取** `info`：卷数、各卷名/描述/版本、压缩方式、各卷大小。
- **实时进度**：解包与校验过程显示进度条 + 速度 + ETA。
- **完整 FFI 绑定**：运行时加载 DLL，绑定 wimlib 全部 72 个导出函数；DLL 缺失时给出友好提示而非崩溃。

## 支持的格式

| 格式 | 说明 |
|------|------|
| WIM  | 标准 Windows 映像 |
| ESD  | solid/LZMS 压缩的 WIM（如微软官方下载），与 WIM 通路一致 |
| SWM  | 分卷 WIM，传第一片即可，自动 glob 引用同目录其余分卷 |

## 构建

目标平台 **Windows x86_64**（`x86_64-pc-windows-msvc`）：

```sh
cargo build --release --target x86_64-pc-windows-msvc
```

`build.rs` 会自动把 `vendor/libwim-15.dll` 复制到可执行文件同目录。该 DLL 为官方 1.14.4 自包含构建，仅依赖 5 个 Windows 系统 DLL，无需 VC++ 运行库。

## 用法

```sh
# 读取镜像信息（卷数 / 卷名 / 版本 / 压缩方式 / 大小）
imgtool info <镜像>

# 校验完整性（带进度条；损坏时退出码非 0）
imgtool verify <镜像>

# 解包指定卷到目录（带进度条）
imgtool extract <镜像> --index <N|all> --dest <目录>

# ESD↔WIM 转换 / 重压缩（带进度条）
imgtool convert <镜像> --dest <输出> [--to wim|esd] [--compress lzx|lzms|xpress|none]

# 制作镜像：把目录打包成 WIM/ESD（带进度条）
imgtool capture <源目录> --dest <输出> [--name <卷名>] [--to wim|esd] [--compress lzx|lzms|xpress|none]

# 分卷：把镜像切成 SWM 多片（带进度条）
imgtool split <镜像> --dest <out.swm> --size <每片MiB>

# 合并：把 SWM 分卷合并回 WIM（传任一分卷，自动找齐）
imgtool join <任一.swm> --dest <输出.wim>
```

示例：

```sh
imgtool info install.wim
imgtool verify install.esd
imgtool extract install.wim --index 1 --dest C:\out
imgtool extract install.swm --dest C:\out      # SWM 传第一片，自动合并分卷
imgtool convert install.esd --dest install.wim          # ESD → WIM（默认 LZX）
imgtool convert install.wim --dest install.esd --to esd # WIM → ESD（LZMS solid）
imgtool capture C:\MyApp --dest MyApp.wim --name "MyApp"  # 目录 → WIM
imgtool split install.wim --dest part.swm --size 3800   # 切成 ≤3800MiB/片（FAT32 友好）
imgtool join part.swm --dest install.wim                # 合并回 WIM
```

退出码：成功 `0`，镜像损坏 `2`，其它错误 `1`。

## 项目结构

```
src/
  ffi/         运行时加载 DLL、#[repr(C)] 类型与全部 72 个函数绑定
  wim.rs       安全封装层（RAII 句柄 + open/info/verify/extract）
  error.rs     错误类型与 wimlib 错误码映射
  progress.rs  进度 / 速度 / ETA（indicatif）
  cli.rs       clap 子命令：info / verify / extract
vendor/        libwim-15.dll、libwim.lib、wimlib.h
fixtures/      小体积测试夹具
```

## 测试

```sh
cargo test --target x86_64-pc-windows-msvc
```

包含单元测试（字节格式化、宽字符转换、SWM glob 推断、结构体内存布局回归等）与端到端集成测试（用 `fixtures/` 跑 info/verify/extract，覆盖 WIM/ESD/SWM 及损坏用例）。

## 许可证

LGPL-3.0-or-later（与 wimlib 一致）。`libwim-15.dll` 为不含 NTFS-3G 的官方构建，动态链接对闭源/商用友好；分发时保留 LGPLv3 许可证文本并允许用户替换该 DLL。
