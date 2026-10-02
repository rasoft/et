#!/usr/bin/env bash
# 编译 macOS 通用二进制，并生成 et.app 与 et.dmg。
# 用法：./scripts/build-macos.sh [Debug|Release]
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib/common.sh"
source "$(dirname "${BASH_SOURCE[0]}")/lib/macos-bundle.sh"
et_prepend_path

root="$(et_root)"
cd "${root}"
et_require_rustc

build_type="${1:-Release}"
case "${build_type}" in
  Debug|Release) ;;
  *)
    echo "构建类型只能是 Debug 或 Release。" >&2
    exit 1
    ;;
esac
if [[ "${build_type}" == "Debug" ]]; then
  echo "提示：Qt 5.15.2 二进制通常只有 Release 库，Debug 可能无法链接。" >&2
fi

if ! et_have cmake || ! et_have ninja || ! et_have cargo || ! et_have lipo || ! et_have hdiutil; then
  echo "缺少 cmake、ninja、cargo、lipo 或 hdiutil。请先运行 ./scripts/bootstrap-macos.sh。" >&2
  exit 1
fi

cache="$(et_cache_dir)"
read_root() {
  local file="$1"
  local line
  if [[ -f "$file" ]]; then
    IFS= read -r line < "$file"
    printf '%s\n' "$line"
  fi
}

qt_x64="${ET_QT_ROOT_X86_64:-}"
if [[ -z "$qt_x64" ]]; then
  qt_x64="$(read_root "${cache}/qt-root-x86_64")"
fi
if [[ -z "$qt_x64" ]]; then
  qt_x64="$(et_qt_root)"
fi
qt_arm="${ET_QT_ROOT_ARM64:-}"
if [[ -z "$qt_arm" ]]; then
  qt_arm="$(read_root "${cache}/qt-root-arm64")"
fi
if [[ ! -f "${qt_x64}/lib/cmake/Qt5/Qt5Config.cmake" || ! -f "${qt_arm}/lib/cmake/Qt5/Qt5Config.cmake" ]]; then
  echo "未找到 x86_64 和 arm64 两套 Qt 5.15。请先运行 ./scripts/bootstrap-macos.sh。" >&2
  exit 1
fi

if ! rustup target list --installed | grep -qx 'aarch64-apple-darwin'; then
  echo "缺少 Rust 目标 aarch64-apple-darwin。请先运行 ./scripts/bootstrap-macos.sh。" >&2
  exit 1
fi
if ! rustup target list --installed | grep -qx 'x86_64-apple-darwin'; then
  echo "缺少 Rust 目标 x86_64-apple-darwin。请先运行 ./scripts/bootstrap-macos.sh。" >&2
  exit 1
fi

build_arch() {
  local arch="$1" deploy="$2" qt="$3" dir="$4"
  cmake -G Ninja -S . -B "$dir" \
    -DCMAKE_BUILD_TYPE="${build_type}" \
    -DCMAKE_OSX_ARCHITECTURES="${arch}" \
    -DCMAKE_OSX_DEPLOYMENT_TARGET="${deploy}" \
    "-DCMAKE_PREFIX_PATH=${qt}"
  cmake --build "$dir"
}

et_ensure_lockfile
cargo test --locked --workspace
build_arch x86_64 10.13 "$qt_x64" build/macos-x86_64
build_arch arm64 11.0 "$qt_arm" build/macos-arm64

if [[ "$(uname -m)" == "arm64" ]]; then
  ln -sfn build/macos-arm64/compile_commands.json compile_commands.json
else
  ln -sfn build/macos-x86_64/compile_commands.json compile_commands.json
fi

rm -rf build/macos/stage-x86_64 build/macos/stage-arm64
et_deploy_app "${root}/build/macos-x86_64/et" "$qt_x64" "${root}/build/macos/stage-x86_64/et.app"
et_deploy_app "${root}/build/macos-arm64/et" "$qt_arm" "${root}/build/macos/stage-arm64/et.app"
et_merge_universal_app \
  "${root}/build/macos/stage-x86_64/et.app" \
  "${root}/build/macos/stage-arm64/et.app" \
  "${root}/build/macos/et.app"
et_make_dmg "${root}/build/macos/et.app" "${root}/build/macos/et.dmg"

info="$(lipo -info "${root}/build/macos/et.app/Contents/MacOS/et")"
echo "${info}"
case "${info}" in
  *x86_64*arm64*|*arm64*x86_64*) ;;
  *)
    echo "通用二进制里没有同时包含 x86_64 和 arm64。" >&2
    exit 1
    ;;
esac

echo "已生成："
echo "  ${root}/build/macos/et.app"
echo "  ${root}/build/macos/et.dmg"
