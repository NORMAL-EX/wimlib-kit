# wimlib-kit —— 交给 Claude Code 的起步包

## 里面有什么
```
wimlib-kit/
├── CLAUDE.md          ← 核心：项目规格 + prompt（放仓库根目录，Claude Code 会自动读）
├── test_links.txt     ← 4 个微软官方真实镜像链接（已验证在线），真实大文件测试用
├── vendor/
│   ├── libwim-15.dll  ← 官方 1.14.4 自包含构建（单文件，无第三方依赖，无需 VC++ 运行库）
│   ├── libwim.lib     ← import 库（用 libloading 的话其实用不到，备着）
│   └── wimlib.h       ← 头文件（定义 #[repr(C)] 结构和常量时照它抄字段顺序）
└── fixtures/          ← 小体积测试夹具，调试时几秒跑一轮，不用先下 5GB
    ├── test.wim       ← 2 卷, LZX, 含完整性表
    ├── test.esd       ← 同内容, solid/LZMS（测 ESD 通路）
    ├── test.swm/2/3   ← 3 片分卷（测 SWM）
    └── corrupt.wim    ← 故意损坏，校验必须失败（错误码 13）
```

## 怎么起步
1. 新建一个空 Rust 项目目录，把本包里的 `CLAUDE.md`、`vendor/`、`fixtures/`、`test_links.txt` 全部拷进去。
2. 在该目录打开 Claude Code，第一句直接说：
   > 读 CLAUDE.md，按里面第 7 节的开发顺序开始，从第 1 步 FFI 加载 DLL 的冒烟测试做起。
3. 让它逐步做、每步用 fixtures 验收，最后再用 test_links.txt 里最小的那个 ESD 跑真实测试。

## 几个已经替你确认好的事实（省得 Claude Code 走弯路）
- `libwim-15.dll` 依赖：仅 ADVAPI32 / KERNEL32 / USER32 / msvcrt / ntdll 五个系统 DLL。单文件丢程序目录即可。
- DLL 共导出 72 个 `wimlib_*` 函数，第一阶段只用到约 10 个（CLAUDE.md 第 4 节已列）。
- 最大的坑：Windows 上路径是 UTF-16 宽字符（`wimlib_tchar = wchar_t`），别传 UTF-8。
- 进度+速度：wimlib 回调给 completed_bytes/total_bytes，速度和 ETA 在 Rust 侧自己算。

## 许可证提醒
此 DLL 为不含 NTFS-3G 的官方构建，属 LGPLv3。动态链接（libloading 即是）对闭源/商用友好；
如果你将来要分发，保留 LGPLv3 许可证文本、并允许用户替换该 DLL 即可。
