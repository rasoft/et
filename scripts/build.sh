#!/usr/bin/env bash
# 按当前系统转去对应的构建脚本。
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
case "$(uname -s)" in
  Linux)
    exec "${here}/build-ubuntu.sh" "$@"
    ;;
  Darwin)
    exec "${here}/build-macos.sh" "$@"
    ;;
  *)
    echo "Windows 请运行：powershell -ExecutionPolicy Bypass -File scripts\\build-windows.ps1" >&2
    exit 1
    ;;
esac
