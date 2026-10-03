#!/usr/bin/env bash
# 编译 Ubuntu 上的 et，并打成 .deb。用法：./scripts/build-ubuntu.sh [Debug|Release]
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib/common.sh"
source "$(dirname "${BASH_SOURCE[0]}")/lib/ubuntu-deb.sh"
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

if ! et_have cmake || ! et_have ninja || ! et_have cargo || ! et_have dpkg-deb || ! et_have dpkg-shlibdeps; then
  echo "缺少 cmake、ninja、cargo、dpkg-deb 或 dpkg-shlibdeps。请先运行 ./scripts/bootstrap-ubuntu.sh。" >&2
  exit 1
fi

cmake_args=(-G Ninja -S . -B build/ubuntu -DCMAKE_BUILD_TYPE="${build_type}")
qt_root="$(et_qt_root || true)"
if [[ -n "${qt_root}" ]]; then
  cmake_args+=("-DCMAKE_PREFIX_PATH=${qt_root}")
fi

et_ensure_lockfile
cargo test --locked --workspace
cmake "${cmake_args[@]}"
cmake --build build/ubuntu
ln -sfn build/ubuntu/compile_commands.json compile_commands.json

deb="$(et_make_deb "${root}/build/ubuntu/et" "${root}/build/ubuntu")"
echo "已生成："
echo "  ${root}/build/ubuntu/et"
echo "  ${deb}"
