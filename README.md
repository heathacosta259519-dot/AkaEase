# AkaEase

A modern, fast, and native Linux desktop music client.

## 特性

- **现代沉浸式界面**：基于 React 19、TypeScript 与 Tailwind CSS 构建，支持高清黑胶唱片旋转动效与双语同步歌词；
- **全屏纯净体验**：无边框无干扰沉浸式舞台，支持窗口任意区域原生拖拽；
- **极致流畅度**：针对高刷显示器（144Hz / 165Hz / 240Hz+）深度优化的硬件加速与视口裁剪管线，滚动稳定 150fps+；
- **原生 Linux 系统集成**：集成 GStreamer 高性能音频引擎与 Linux MPRIS 系统级媒体控制。

## 构建与运行

### 前置依赖

- Node.js (v18+) 与 npm
- Rust 与 Cargo (最新稳定版)
- GStreamer 开发库与 WebKitGTK

### 构建命令

```bash
./build.sh
```

构建完成后产物将输出至 `dist/` 目录。
