# 产品方案

本目录用于存放声记的产品定位、用户流程、PRD 和功能边界说明。

## 当前核心路线

- [声记-版本迭代与项目架构方案](../02-技术方案/声记-版本迭代与项目架构方案.md)
- [声记-版本迭代目标与代码归档方案](../03-版本迭代/声记-版本迭代目标与代码归档方案.md)

## ⚠️ 凭据管理规范

**严禁**在仓库任何位置保存明文 API Key / Secret / Token。

### 凭据存储

| 凭据类型 | 存储方式 | 读取方式 |
| --- | --- | --- |
| MiniMax-M3 API Key | macOS Keychain（`security` 命令） | `security find-generic-password -s shengji-mac -a minimax-m3-api-key -w` |
| 本地 WhisperKit / SpeakerKit 模型路径 | 用户配置目录 `~/Library/Application Support/shengji-mac/config.json` | 通过 Tauri `app_config` command 读取 |
| 数据库密码（如启用） | macOS Keychain | `security find-generic-password -s shengji-mac-db` |

### 凭据命名约定

- 服务名（service）：`shengji-mac`
- 账户名（account）：`<provider>-<purpose>`，例如：
  - `minimax-m3-api-key`
  - `whisperkit-model-token`（如有）

### Keychain 写入命令示例

```bash
# 写入（首次设置，需手工执行，不进任何脚本或文档）
security add-generic-password \
  -a "minimax-m3-api-key" \
  -s "shengji-mac" \
  -w "<从 provider 控制台获取的 key>"

# 应用读取（在 Rust 中通过 keyring crate 调用）
# 或在调试时手动验证：
security find-generic-password \
  -a "minimax-m3-api-key" \
  -s "shengji-mac" -w
```

### 误提交应急

如果 key 已被 `git add` / `git commit`：
1. **立即**在 provider 控制台 **revoke** 旧 key 并重新签发
2. 用 `git filter-repo` 或 BFG 改写历史
3. force-push 前通知所有协作者
4. 同步审查 GitHub Secret Scanning / GitGuardian 告警

## AI Loop / Agent 协作守则

任何 AI 助手 / Agent / Cron Job 在产出声记相关文档时：
- 禁止输出真实 API Key 到任何 `.md` / `.json` / 代码文件
- 引用凭据时仅写「从 Keychain 读取」+ 账户名
- 7 项验收中加一项：**`grep -rE "sk-(cp-)?[A-Za-z0-9]{30,}" .` 必须返回 0 命中**（除 .gitignore / Keychain 说明文档）
