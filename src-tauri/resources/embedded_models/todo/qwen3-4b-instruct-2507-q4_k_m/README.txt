当前仓库仅提交 Qwen3-4B-Instruct-2507 Q4_K_M 的运行时清单和 Prompt 模板。

要使内嵌本地 Todo 运行时进入 ready 状态，需要在应用数据目录对应版本下放入以下文件：
1. bin/llama-cli
2. weights/qwen3-4b-instruct-2507-q4_k_m.gguf

开发调试时也可通过以下环境变量覆盖：
1. SMART_TODO_LLAMA_CLI_PATH
2. SMART_TODO_QWEN_GGUF_PATH

正式打包时，这两个文件应随 App 一起分发，并在首次启动时释放到本地模型目录。
