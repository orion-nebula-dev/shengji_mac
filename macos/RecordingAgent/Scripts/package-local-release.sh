#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
REPO_DIR="$(cd "$ROOT_DIR/../.." && pwd)"
VERSION="1.0.0"
DATE_STAMP="${DATE_STAMP:-$(date +%F)}"
ARCHIVE_ROOT="$REPO_DIR/其他文件/build/v$VERSION"
BUILD_DIR="$ARCHIVE_ROOT/$DATE_STAMP"
if [[ -e "$BUILD_DIR" ]]; then
  suffix=2
  while [[ -e "$ARCHIVE_ROOT/$DATE_STAMP-$suffix" ]]; do
    suffix=$((suffix + 1))
  done
  BUILD_DIR="$ARCHIVE_ROOT/$DATE_STAMP-$suffix"
fi
RELEASE_DIR="$BUILD_DIR/release-package"
CHECKSUM_DIR="$BUILD_DIR/checksums"
STAGING_DIR="$(mktemp -d "${TMPDIR:-/tmp}/recording-agent-package.XXXXXX")"
APP_DIR="$STAGING_DIR/RecordingAgent.app"
EXECUTABLE="$ROOT_DIR/.build/release/RecordingAgent"
ZIP_PATH="$RELEASE_DIR/RecordingAgent-v$VERSION-local.zip"
CHECKSUM_PATH="$CHECKSUM_DIR/checksums.sha256"
NOTES_PATH="$BUILD_DIR/notes.md"
EXTRACT_DIR="$STAGING_DIR/extracted"

cleanup() {
  rm -rf "$STAGING_DIR"
}
trap cleanup EXIT

clear_app_extended_attributes() {
  local target="$1"
  xattr -cr "$target" 2>/dev/null || true
  xattr -d com.apple.FinderInfo "$target" 2>/dev/null || true
  xattr -d 'com.apple.fileprovider.fpfs#P' "$target" 2>/dev/null || true
  xattr -d com.apple.provenance "$target" 2>/dev/null || true
}

swift build --package-path "$ROOT_DIR" -c release --product RecordingAgent

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
cp "$EXECUTABLE" "$APP_DIR/Contents/MacOS/RecordingAgent"
chmod +x "$APP_DIR/Contents/MacOS/RecordingAgent"

cat > "$APP_DIR/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>zh_CN</string>
  <key>CFBundleExecutable</key>
  <string>RecordingAgent</string>
  <key>CFBundleIdentifier</key>
  <string>com.shengji.recording-agent</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>声记</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSMicrophoneUsageDescription</key>
  <string>声记需要在用户主动开始记录后使用麦克风保存本地 WAV 并进行本地转写。</string>
  <key>NSPrincipalClass</key>
  <string>NSApplication</string>
</dict>
</plist>
PLIST

clear_app_extended_attributes "$APP_DIR"
codesign --force --sign - "$APP_DIR"
codesign --verify --deep --strict "$APP_DIR"

mkdir -p "$RELEASE_DIR" "$CHECKSUM_DIR"
ditto -c -k --norsrc --keepParent "$APP_DIR" "$ZIP_PATH"
unzip -t "$ZIP_PATH" >/dev/null
mkdir -p "$EXTRACT_DIR"
ditto -x -k "$ZIP_PATH" "$EXTRACT_DIR"
clear_app_extended_attributes "$EXTRACT_DIR/RecordingAgent.app"
codesign --verify --deep --strict "$EXTRACT_DIR/RecordingAgent.app"

shasum -a 256 "$ZIP_PATH" > "$CHECKSUM_PATH"

cat > "$NOTES_PATH" <<EOF
# RecordingAgent v$VERSION local package

- Build date: $(date -u +"%Y-%m-%dT%H:%M:%SZ")
- Source: macos/RecordingAgent
- Command: macos/RecordingAgent/Scripts/package-local-release.sh
- Zip: release-package/$(basename "$ZIP_PATH")
- Checksum: checksums/$(basename "$CHECKSUM_PATH")

## Verification

\`\`\`bash
swift build --package-path macos/RecordingAgent -c release --product RecordingAgent
unzip -t "$ZIP_PATH"
rm -rf /tmp/recording-agent-package-check
mkdir -p /tmp/recording-agent-package-check
ditto -x -k "$ZIP_PATH" /tmp/recording-agent-package-check
xattr -cr /tmp/recording-agent-package-check/RecordingAgent.app 2>/dev/null || true
xattr -d com.apple.FinderInfo /tmp/recording-agent-package-check/RecordingAgent.app 2>/dev/null || true
xattr -d 'com.apple.fileprovider.fpfs#P' /tmp/recording-agent-package-check/RecordingAgent.app 2>/dev/null || true
xattr -d com.apple.provenance /tmp/recording-agent-package-check/RecordingAgent.app 2>/dev/null || true
codesign --verify --deep --strict /tmp/recording-agent-package-check/RecordingAgent.app
shasum -a 256 -c "$CHECKSUM_PATH"
\`\`\`

## Known Limits

- Local ad-hoc signed package for development validation, not notarized.
- The archived deliverable is the zip. A bare app is verified in temporary staging and not retained in the build archive because local file-provider metadata can invalidate strict codesign checks on app bundles stored under Documents.
- Model files, recordings, SQLite databases, and API keys are not bundled.
EOF

echo "$ZIP_PATH"
echo "$CHECKSUM_PATH"
echo "$NOTES_PATH"
