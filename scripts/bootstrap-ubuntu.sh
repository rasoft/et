#!/usr/bin/env bash
# 建立 Ubuntu 构建环境：Rust 1.77.2、CMake、Ninja、发行版里的 Qt 5.15 开发包。
# 可重复执行。用法：./scripts/bootstrap-ubuntu.sh
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib/common.sh"

if [[ -r /etc/os-release ]]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  ok=0
  [[ "${ID:-}" == "ubuntu" || "${ID:-}" == "debian" ]] && ok=1
  [[ "${ID_LIKE:-}" == *debian* || "${ID_LIKE:-}" == *ubuntu* ]] && ok=1
  if [[ "${ok}" -ne 1 ]]; then
    echo "这个脚本面向 Ubuntu / Debian（apt）。当前是 ${ID:-unknown}。" >&2
    exit 1
  fi
fi

export DEBIAN_FRONTEND=noninteractive
packages=(
  build-essential
  ca-certificates
  cmake
  curl
  dpkg-dev
  ninja-build
  pkg-config
  qtbase5-dev
)
if [[ "$(id -u)" -eq 0 ]]; then
  apt-get update
  apt-get install -y "${packages[@]}"
else
  sudo apt-get update
  sudo apt-get install -y "${packages[@]}"
fi

qt_ver="$(pkg-config --modversion Qt5Core)"
echo "Qt ${qt_ver}"
case "${qt_ver}" in
  5.15.*) ;;
  *)
    echo "需要 Qt 5.15，当前是 ${qt_ver}。" >&2
    exit 1
    ;;
esac

qt_prefix="$(pkg-config --variable=prefix Qt5Core)"
cache="$(et_cache_dir)"
mkdir -p "${cache}"
printf '%s\n' "${qt_prefix}" > "${cache}/qt-root"

et_ensure_rust

echo "Ubuntu 构建环境已就绪。"
echo "Qt 前缀：${qt_prefix}"
echo "下一步：./scripts/build-ubuntu.sh"
