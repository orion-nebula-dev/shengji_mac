---
applyTo: "src/types.ts"
---

# TypeScript 类型规范

## 规则

1. 所有共享类型集中维护在 `src/types.ts`。
2. 类型要与 `src/lib/desktop.ts` 和 Rust command 返回结构保持一致。
3. 不使用 `any`。
4. 时间字段当前可继续使用字符串；展示层负责格式化。
5. 可空响应使用 `string | null`，可省略请求字段使用可选属性。

## 当前核心类型

- `SettingsState`
- `TodoItem`
- `SessionItem`
- `RuntimeStatus`
- `LocalRuntimeState`

## 变更要求

- 新增字段后必须更新 mock 数据、桌面接口映射和 UI 展示逻辑。
- 如果字段涉及持久化或接口契约，同步更新 `AI文档/OpenAPI接口契约.md`。
