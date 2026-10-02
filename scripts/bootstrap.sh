#!/usr/bin/env bash
# 按当前系统转去对应的环境脚本。
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
case "$(uname -s)" in
  Linux)
    exec "${here}/bootstrap-ubuntu.sh" "$@"
    ;;
  Darwin)
    exec "${here}/bootstrap-macos.sh" "$@"
    ;;
  *)
    echo "Windows 请运行：powershell -ExecutionPolicy Bypass -File scripts\\bootstrap-windows.ps1" >&2
    exit 1
    ;;
esac
