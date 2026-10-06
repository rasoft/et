# eMMC Toolkit

跨平台 eMMC 镜像包编辑与下载工具。面向固件与量产工程师，在 Windows、macOS、Ubuntu 上用同一套程序维护 eMMC 分区表和分区镜像，并在后续阶段把包下载到设备。

当前可以编译出一个空白主窗口。分区编辑还没做。

## 文档

| 文档 | 内容 |
| --- | --- |
| [需求](docs/requirements.md) | 用户、场景、功能与非功能需求、范围边界 |
| [架构](docs/architecture.md) | 技术选型、分层、编辑模型、下载扩展点 |
| [包格式](docs/package-format.md) | `.etpk` 文件与工作副本目录、`manifest.json` 规范 |
| [路线图](docs/roadmap.md) | 里程碑、交付物与建议的实现顺序 |

## 构建

三个平台都先建立环境，再编译。环境脚本可以重复执行。工具链钉在 Rust 1.77.2 和 Qt 5.15。依赖装在用户缓存目录，不进仓库。

| 平台 | 建立环境 | 编译 |
| --- | --- | --- |
| Ubuntu 22.04 | `./scripts/bootstrap-ubuntu.sh` | `./scripts/build-ubuntu.sh` |
| macOS | `./scripts/bootstrap-macos.sh` | `./scripts/build-macos.sh` |
| Windows x64 | `powershell -ExecutionPolicy Bypass -File scripts\bootstrap-windows.ps1` | `powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1` |

macOS 产物是同时包含 x86_64 和 arm64 的 `build/macos/et.app` 和 `build/macos/et.dmg`。Ubuntu 产物是 `build/ubuntu/et_<version>_<arch>.deb`，安装后使用发行版里的 Qt 5.15 Widgets 运行库。Windows 构建机使用 Visual Studio 2019（MSVC v142）；可执行文件的子系统版本是 Windows 7。可选参数 `Debug` 或 `Release`，默认 `Release`。官方下载站超时时，可以设置 `ET_QT_MIRROR` 指向带 `online` 目录的 Qt 镜像后再跑环境脚本。

## 已定结论

- **产品形态**：桌面程序。编辑发生在临时目录里的工作副本，保存和打开的是单个 `.etpk` 文件，而不是把整颗 eMMC 载入内存。
- **第一期范围**：新建、打开、保存镜像包；增删分区、重命名、改大小、更换分区镜像；保存前做布局校验。
- **技术栈**：Rust 核心库 `et-core` + Qt 5.15 界面。界面只发编辑命令，分区算术和包读写都在 Rust 里完成。Windows 最低支持 Windows 7 SP1 x64，因此不用依赖 WebView2 的 Tauri。
- **下载**：不进第一期。包格式和核心库按“镜像始终是可流式读取的文件”来设计，避免以后为 USB 下载改格式。

## 名字

仓库和产品代号使用 **et**。用户文件后缀是 `.etpk`。清单里的格式字段仍是 `format: "etpack"`。
