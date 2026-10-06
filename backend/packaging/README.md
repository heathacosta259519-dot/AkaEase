# Linux 发行与运行依赖

这是动态链接的 Linux 应用。后端核心不依赖前端源码，最终桌面发布构建需要前端提供 `frontend/dist/index.html` 及其资源。

在 `backend/` 执行：

```sh
python scripts/doctor.py --build
python scripts/package_linux.py --check
python scripts/package_linux.py --offline
```

打包脚本在前端产物缺失时明确失败；产物存在后，编译 `custom-protocol` release，将真实前端嵌入 Tauri，生成带 SHA-256 校验文件的 tar.gz。不会把 Vite 开发宿主作为发行版本。当前架构取构建机架构，不提供交叉编译包装。构建目录及归档在后端工作区，重复文件名不覆盖。

包内容采用 `usr/bin`、`usr/share/applications`、`usr/share/icons`、`usr/share/doc` 布局。可直接执行解压后的 `usr/bin/akanetease-desktop`；需要菜单入口时，由发行版包管理或用户自行将对应文件安装到系统/用户路径。卸载只删除这些程序文件，不自动删除 XDG 状态或钥匙串。

| 用途 | Arch Linux | Debian/Ubuntu（包名随版本变化） |
| --- | --- | --- |
| 编译 | rust、base-devel、pkgconf | rustup/Rust 1.89+ 工具链、build-essential、pkg-config |
| 桌面开发库 | gtk3、webkit2gtk-4.1、librsvg | libgtk-3-dev、libwebkit2gtk-4.1-dev、librsvg2-dev |
| 音频开发库 | gstreamer | libgstreamer1.0-dev |
| 桌面运行库 | gtk3、webkit2gtk-4.1、librsvg | libgtk-3-0、libwebkit2gtk-4.1-0、librsvg2-2 |
| 音频运行/解码 | gstreamer、gst-plugins-base、gst-plugins-good、可选 gst-libav | gstreamer1.0-plugins-base、gstreamer1.0-plugins-good、可选 gstreamer1.0-libav |
| 音频输出 | PipeWire/PulseAudio 或 ALSA，及相应插件 | 当前桌面音频服务及对应 GStreamer 输出插件 |
| 凭据保存 | libsecret + GNOME Keyring 或兼容服务 | libsecret-tools + gnome-keyring 或兼容服务 |
| 系统媒体控制 | dbus | dbus-user-session |

运行需要正常的图形会话、会话 D-Bus 和系统 CA 证书。Secret Service 不可用时账号降级为内存会话；MPRIS 不可用时应用内播放仍能工作。`doctor.py` 只检查命令/插件/开发库和 D-Bus 连接，不读取凭据、不访问网易云，也不证明硬件输出正常。MP3 如使用其他等价解码插件，可人工确认诊断提示。

发布前仍需在目标发行版验证 WebView、音频输出、真实账号、钥匙串和媒体键。构建机 libc 与目标系统的兼容性必须匹配，建议在最旧支持系统中构建。本项目许可证尚待维护者确定；发行方需携带选定许可证及依赖所需的声明。现已基于前端 `268374d` 产物生成 Linux x86_64 包并在本机验证启动、MPRIS 和退出保存；真实账号和其他发行版仍待验收。
