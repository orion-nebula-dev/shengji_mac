#!/usr/bin/env bash
set -euo pipefail

echo "声记发布检查"
echo "1. npm run build"
npm run build

echo "2. cargo check"
cargo check --manifest-path src-tauri/Cargo.toml

echo "发布检查完成。请人工确认发布说明、tag 和推送目标。"
