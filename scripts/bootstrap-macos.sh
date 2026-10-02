#!/usr/bin/env bash
# 建立 macOS 构建环境：Xcode 命令行工具、Rust 1.77.2、CMake、Ninja、
# 官方 Qt 5.15.2（x86_64）和带苹果芯片支持的 Qt 5.15.2（arm64）。可重复执行。
# 用法：./scripts/bootstrap-macos.sh
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/lib/common.sh"

if ! xcode-select -p >/dev/null 2>&1; then
  xcode-select --install || true
  echo "请先装好 Xcode 命令行工具，然后重新运行本脚本。" >&2
  exit 1
fi

if ! et_have python3; then
  echo "需要 python3（随 Xcode 命令行工具提供）。" >&2
  exit 1
fi

et_ensure_rust
rustup target add x86_64-apple-darwin --toolchain 1.77.2
rustup target add aarch64-apple-darwin --toolchain 1.77.2

cache="$(et_cache_dir)"
venv="${cache}/venv"
mkdir -p "${cache}"
if [[ ! -x "${venv}/bin/python" ]]; then
  python3 -m venv "${venv}"
fi
if [[ ! -x "${venv}/bin/aqt" || ! -x "${venv}/bin/cmake" || ! -x "${venv}/bin/ninja" ]]; then
  "${venv}/bin/python" -m pip install --upgrade pip
  "${venv}/bin/python" -m pip install 'aqtinstall>=3.1,<4' cmake ninja
fi
# 系统 Python 往往链的是 LibreSSL。urllib3 2 在那上面会下坏 Qt 包。
if ! "${venv}/bin/python" -c 'import urllib3; raise SystemExit(0 if int(urllib3.__version__.split(".")[0]) < 2 else 1)'; then
  "${venv}/bin/python" -m pip install 'urllib3<2'
fi

qt_root="${cache}/qt/5.15.2/clang_64"
if [[ ! -f "${qt_root}/lib/cmake/Qt5/Qt5Config.cmake" ]]; then
  echo "下载 Qt 5.15.2 clang_64（只要 qtbase）"
  aqt_args=(install-qt mac desktop 5.15.2 clang_64 --outputdir "${cache}/qt" --archives qtbase --timeout 300)
  if [[ -n "${ET_QT_MIRROR:-}" ]]; then
    aqt_args+=(-b "${ET_QT_MIRROR}")
  fi
  downloaded=0
  for _attempt in 1 2 3; do
    if (cd "${cache}" && "${venv}/bin/aqt" "${aqt_args[@]}"); then
      downloaded=1
      break
    fi
    echo "Qt 下载失败，重试 ${_attempt}/3。若镜像一直超时，可设置 ET_QT_MIRROR 后重跑。" >&2
    sleep 2
  done
  if [[ "${downloaded}" -ne 1 ]]; then
    exit 1
  fi
fi
if [[ ! -f "${qt_root}/lib/cmake/Qt5/Qt5Config.cmake" ]]; then
  echo "Qt 已下载，但没找到 ${qt_root}/lib/cmake/Qt5/Qt5Config.cmake。" >&2
  exit 1
fi
printf '%s\n' "${qt_root}" > "${cache}/qt-root"
printf '%s\n' "${qt_root}" > "${cache}/qt-root-x86_64"

# 官方 5.15.2 没有 arm64 包。这是 Kitware 公开的 Qt 5.15.2 arm64 构建，
# 与 CMake / ParaView 使用的是同一份。
arm_archive="${cache}/downloads/qt-5.15.2-macosx11.0-arm64.tar.xz"
arm_url="https://gitlab.kitware.com/api/v4/projects/6955/packages/generic/qt/v5.15.2-20210519.0/qt-5.15.2-macosx11.0-arm64.tar.xz"
arm_sha="e80d2647de461370bb65db60bc657148d196348b7393a2975d4a54bfba1b217f"
arm_root=""
if [[ -f "${cache}/qt-root-arm64" ]]; then
  IFS= read -r arm_root < "${cache}/qt-root-arm64"
fi
if [[ ! -f "${arm_root}/lib/cmake/Qt5/Qt5Config.cmake" ]]; then
  mkdir -p "${cache}/downloads"
  if [[ ! -f "${arm_archive}" ]] || ! shasum -a 256 "${arm_archive}" | grep -q "${arm_sha}"; then
    echo "下载带苹果芯片支持的 Qt 5.15.2（arm64）"
    curl --http1.1 -L --fail --retry 5 --retry-all-errors -C - \
      -o "${arm_archive}" "${arm_url}"
  fi
  actual_sha="$(shasum -a 256 "${arm_archive}" | awk '{print $1}')"
  if [[ "${actual_sha}" != "${arm_sha}" ]]; then
    echo "arm64 Qt 校验失败：${actual_sha}" >&2
    exit 1
  fi
  rm -rf "${cache}/qt-arm64-extract"
  mkdir -p "${cache}/qt-arm64-extract"
  tar -xJf "${arm_archive}" -C "${cache}/qt-arm64-extract"
  arm_config="$(find "${cache}/qt-arm64-extract" -name Qt5Config.cmake -print -quit)"
  if [[ -z "${arm_config}" ]]; then
    echo "arm64 Qt 包里没有 Qt5Config.cmake。" >&2
    exit 1
  fi
  arm_root="$(cd "$(dirname "${arm_config}")/../../.." && pwd)"
  printf '%s\n' "${arm_root}" > "${cache}/qt-root-arm64"
fi

if [[ "$(uname -m)" == "arm64" ]]; then
  if ! arch -x86_64 /usr/bin/true >/dev/null 2>&1; then
    echo "安装 Rosetta。Qt 5.15 的 moc 和 x86_64 产物都要靠它运行。"
    softwareupdate --install-rosetta --agree-to-license
  fi
fi

echo "macOS 构建环境已就绪。"
echo "Qt x86_64：${qt_root}"
echo "Qt arm64：$(cat "${cache}/qt-root-arm64")"
echo "下一步：./scripts/build-macos.sh"
