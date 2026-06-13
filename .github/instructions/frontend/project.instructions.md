---
applyTo: "src/**/*.ts, src/**/*.tsx, vite.config.ts, tsconfig*.json, package.json"
---

# 前端项目规范

## 技术栈

- React 19 + TypeScript + Vite。
- 当前不使用 Tailwind、Zustand、React Router、Axios 或组件库。
- Tauri bridge 封装在 `src/lib/desktop.ts`，浏览器原型兜底数据在 `src/data/mock.ts` 与 `src/lib/storage.ts`。

## 目录职责

```text
src/
├── App.tsx        # 当前主界面与页面状态
├── main.tsx       # React 入口
├── styles.css     # 全局样式与设计 token
├── types.ts       # 共享类型
├── data/mock.ts   # 浏览器原型兜底数据
└── lib/           # Tauri command 与本地状态封装
```

## 开发规则

1. 用户可见文案使用中文。
2. 新增类型优先放在 `src/types.ts`。
3. 前端只通过 `src/lib/desktop.ts` 访问 Tauri command，不在组件里直接拼接 `window.__TAURI__` 细节。
4. 保留浏览器原型兜底能力；Tauri 不可用时展示明确说明，而不是空白或崩溃。
5. 当前版本以单页应用为主，不新增路由系统，除非需求明确。
6. 异步操作必须有可见反馈，如顶部提示、按钮禁用或状态卡。

## 验证

- 前端改动后运行 `npm run build`。
