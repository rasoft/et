# 被 build-ubuntu.sh source。把已编译的 et 打成依赖系统 Qt 5 Widgets 的 .deb。

et_deb_version() {
  local root ver
  root="$(et_root)"
  ver="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "${root}/Cargo.toml" | head -n 1)"
  if [[ -z "${ver}" ]]; then
    echo "无法从 Cargo.toml 读取版本。" >&2
    return 1
  fi
  printf '%s\n' "${ver}"
}

et_deb_shlib_depends() {
  local bin="$1"
  local work line
  work="$(mktemp -d)"
  mkdir -p "${work}/debian"
  printf '%s\n' 'Source: et' '' 'Package: et' 'Architecture: any' > "${work}/debian/control"
  line="$(cd "${work}" && dpkg-shlibdeps -O "${bin}")"
  rm -rf "${work}"
  line="${line#shlibs:Depends=}"
  if [[ -z "${line}" ]]; then
    echo "dpkg-shlibdeps 没有给出依赖。" >&2
    return 1
  fi
  case "${line}" in
    *libqt5widgets5*) ;;
    *)
      echo "运行库依赖里没有 libqt5widgets5：${line}" >&2
      return 1
      ;;
  esac
  case "${line}" in
    *webkit*|*webkitgtk*)
      echo "运行库依赖不应包含 WebKit：${line}" >&2
      return 1
      ;;
  esac
  printf '%s\n' "${line}"
}

et_make_deb() {
  local bin="$1"
  local dest_dir="$2"
  if [[ ! -f "${bin}" ]]; then
    echo "找不到可执行文件：${bin}" >&2
    return 1
  fi
  if ! et_have dpkg-deb || ! et_have dpkg-shlibdeps || ! et_have strip; then
    echo "缺少 dpkg-deb、dpkg-shlibdeps 或 strip。请先运行 ./scripts/bootstrap-ubuntu.sh。" >&2
    return 1
  fi

  # 子 shell 把 umask 和 dpkg-deb 的提示留在打包过程里。
  (
    umask 022
    set -euo pipefail
    local root version arch stage desktop depends size deb info contents
    root="$(et_root)"
    version="$(et_deb_version)"
    arch="$(dpkg --print-architecture)"
    desktop="${root}/src-qt/linux/et.desktop"
    stage="${dest_dir}/deb-root"
    deb="${dest_dir}/et_${version}_${arch}.deb"

    if [[ ! -f "${desktop}" ]]; then
      echo "找不到桌面项：${desktop}" >&2
      exit 1
    fi

    rm -rf "${stage}"
    mkdir -p "${stage}/DEBIAN" "${stage}/usr/bin" "${stage}/usr/share/applications"
    install -m 755 "${bin}" "${stage}/usr/bin/et"
    install -m 644 "${desktop}" "${stage}/usr/share/applications/et.desktop"

    depends="$(et_deb_shlib_depends "${stage}/usr/bin/et")"
    strip --strip-unneeded "${stage}/usr/bin/et"
    size="$(du -sk "${stage}/usr" | awk '{print $1}')"

    (
      cd "${stage}"
      find usr -type f -print0 | sort -z | xargs -0 md5sum
    ) > "${stage}/DEBIAN/md5sums"

    cat > "${stage}/DEBIAN/control" <<EOF
Package: et
Version: ${version}
Section: utils
Priority: optional
Architecture: ${arch}
Maintainer: et <et@localhost>
Installed-Size: ${size}
Depends: ${depends}
Description: eMMC 镜像包编辑工具
 编辑 eMMC 分区表和分区镜像。
EOF

    rm -f "${deb}"
    dpkg-deb --root-owner-group --build "${stage}" "${deb}" >&2
    rm -rf "${stage}"

    info="$(dpkg-deb --info "${deb}")"
    case "${info}" in
      *"Package: et"* ) ;;
      *)
        echo "生成的包元数据异常。" >&2
        exit 1
        ;;
    esac
    contents="$(dpkg-deb --contents "${deb}")"
    case "${contents}" in
      *'./usr/bin/et'*) ;;
      *)
        echo "包里没有 /usr/bin/et。" >&2
        exit 1
        ;;
    esac

    printf '%s\n' "${deb}"
  )
}
