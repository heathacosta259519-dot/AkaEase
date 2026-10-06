# Frontend workspace

前端 Agent 的独立工作区。界面、交互、视觉资源以及前端专属构建和测试文件放在这里。

技术栈：React 19 + TypeScript + Vite + Tailwind CSS + Tauri 2 IPC。

## 目录结构

- `src/types/backend.ts`：与 Rust 后端对齐的完整类型定义。
- `src/services/api.ts`：Tauri IPC 命令调用与全局事件监听封装层。
- `src/App.tsx`：主应用界面。
- `BACKEND_API.md`：详细的后端 IPC 接口参考手册。

## 构建与开发

```bash
npm install
npm run dev   # 启动 Vite 开发服务器 (localhost:5173)
npm run build # 构建生产前端产物 (dist/)
```

