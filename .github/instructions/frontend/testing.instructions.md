---
applyTo: "src/**/*.test.ts, src/**/*.test.tsx, vitest.config.ts"
---

# 前端测试规范

当前项目尚未配置前端测试框架。新增测试前需先确认依赖与脚本。

## 推荐方向

- 测试框架：Vitest。
- 组件测试：Testing Library。
- 重点覆盖：
  - Todo 状态切换。
  - 设置保存。
  - 浏览器原型兜底。
  - Tauri command 失败提示。
  - 会话日志与失败状态展示。

## 当前最小验证

没有测试框架时，前端改动必须运行：

```bash
npm run build
```
