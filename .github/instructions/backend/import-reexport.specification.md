---
applyTo: "src-tauri/src/**/*.rs, src/**/*.ts"
---

# 导入与模块暴露规范

## TypeScript

- 项目当前没有路径别名，保持相对导入即可。
- 类型导入使用 `import type`。
- 避免跨层导入 Tauri 底层细节；前端统一通过 `src/lib/desktop.ts`。

## Rust

- 模块暴露保持最小可见性。
- command 层只暴露前端需要的稳定函数。
- 录音、存储、模型运行时等内部模块不要通过 UI 侧直接耦合。
