---
applyTo: "src/App.tsx"
---

# 页面与导航规范

## 当前约定

- 当前应用是 Tauri 单页界面，不使用 React Router。
- 页面切换通过本地 state 控制。
- 顶层导航应贴近声记 v2.0：`Now / Today / Actions / History / System / Settings` 的语义可以逐步映射到当前功能。

## 页面职责映射

| 设计页 | 当前版本映射 |
| --- | --- |
| Now / Today | 主工作台中的录音状态、Todo 概览 |
| Actions | Todo 列表、状态切换、详情 |
| History | 会话文稿、来源文本、提取结果 |
| SystemStates | 运行状态、失败原因、本地模型状态、回退路径 |
| Settings | 录音、ASR、Todo 提取、本地模型配置 |

## 规则

1. 不新增路由依赖，除非需求明确。
2. 导航文案使用中文。
3. 页面切换不能丢失当前已加载的 Todo、会话和设置状态。
4. 系统状态和日志入口应常驻可见，不能只藏在设置页。
