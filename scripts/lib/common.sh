# 被 bootstrap / build 脚本 source。不要直接执行。

_ET_COMMON_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

et_root() {
  (cd "${_ET_COMMON_DIR}/../.." && pwd)
}

et_cache_dir() {
  printf '%s\n' "${ET_CACHE_DIR:-${HOME}/.cache/et}"
}

et_prepend_path() {
  local cache cargo_home
  cache="$(et_cache_dir)"
  cargo_home="${CARGO_HOME:-${HOME}/.cargo}"
  PATH="${cache}/venv/bin:${cargo_home}/bin:${PATH}"
  export PATH
}

et_have() {
  command -v "$1" >/dev/null 2>&1
}

et_ensure_rust() {
  local cargo_home
  cargo_home="${CARGO_HOME:-${HOME}/.cargo}"
  if [[ ! -x "${cargo_home}/bin/rustup" ]]; then
    echo "安装 rustup，并带上 Rust 1.77.2"
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain 1.77.2 --profile minimal --component rustfmt,clippy
  fi
  # shellcheck disable=SC1091
  source "${cargo_home}/env"
  rustup toolchain install 1.77.2 --profile minimal --component rustfmt,clippy
}

et_qt_root() {
  if [[ -n "${ET_QT_ROOT:-}" ]]; then
    printf '%s\n' "${ET_QT_ROOT}"
    return
  fi
  local file line
  file="$(et_cache_dir)/qt-root"
  if [[ -f "${file}" ]]; then
    IFS= read -r line < "${file}"
    printf '%s\n' "${line}"
  fi
}

et_ensure_lockfile() {
  if [[ ! -f Cargo.lock ]]; then
    cargo generate-lockfile
  fi
}

et_require_rustc() {
  if ! et_have rustc; then
    echo "未找到 rustc。请先运行对应平台的 bootstrap 脚本。" >&2
    exit 1
  fi
  local ver
  ver="$(rustc --version)"
  case "${ver}" in
    "rustc 1.77."*) ;;
    *)
      echo "需要 Rust 1.77，当前是：${ver}" >&2
      echo "请在仓库根目录构建，让 rustup 按 rust-toolchain.toml 选择 1.77.2。" >&2
      exit 1
      ;;
  esac
}

et_require_qt_root() {
  local root
  root="$(et_qt_root)"
  if [[ -z "${root}" || ! -f "${root}/lib/cmake/Qt5/Qt5Config.cmake" ]]; then
    echo "未找到 Qt 5.15。请先运行对应平台的 bootstrap 脚本，或设置 ET_QT_ROOT。" >&2
    exit 1
  fi
  printf '%s\n' "${root}"
}
