# 被 build-macos.sh source。把两个单架构可执行文件收成通用 et.app 和 dmg。

et_is_macho() {
  file "$1" | grep -q 'Mach-O'
}

et_framework_id() {
  # 从依赖路径里取出 QtCore.framework/Versions/5/QtCore 这种相对 id。
  printf '%s\n' "$1" | sed -n 's#.*\(Qt[^/]*\.framework/Versions/[^/]*/[^/]*\)$#\1#p'
}

et_collect_frameworks() {
  local bin="$1" qt_lib="$2" dest_fw="$3"
  local dep fw src real name
  local deps
  deps="$(otool -L "$bin" | awk 'NR>1 { print $1 }')"
  for dep in $deps; do
    fw="$(et_framework_id "$dep")"
    if [[ -z "$fw" ]]; then
      continue
    fi
    name="${fw%%/*}"
    if [[ -d "${dest_fw}/${name}" ]]; then
      continue
    fi
    src="${qt_lib}/${name}"
    if [[ ! -d "$src" ]]; then
      echo "Qt 前缀里没有 ${src}" >&2
      return 1
    fi
    cp -R "$src" "${dest_fw}/${name}"
    real="${dest_fw}/${fw}"
    if [[ ! -f "$real" ]]; then
      echo "框架布局不符合预期：${real}" >&2
      return 1
    fi
    et_collect_frameworks "$real" "$qt_lib" "$dest_fw"
  done
}

et_rewrite_qt_deps() {
  local bin="$1"
  local dep fw deps
  deps="$(otool -L "$bin" | awk 'NR>1 { print $1 }')"
  for dep in $deps; do
    case "$dep" in
      @rpath/*|/System/*|/usr/lib/*) continue ;;
    esac
    fw="$(et_framework_id "$dep")"
    if [[ -n "$fw" ]]; then
      install_name_tool -change "$dep" "@rpath/${fw}" "$bin"
    fi
  done
}

et_rpaths() {
  otool -l "$1" | awk '/cmd LC_RPATH/{f=1} f && $1=="path" {print $2; f=0}'
}

et_ensure_rpath() {
  local bin="$1" rpath="$2"
  if ! et_rpaths "$bin" | grep -qx "$rpath"; then
    install_name_tool -add_rpath "$rpath" "$bin"
  fi
}

et_delete_absolute_rpaths() {
  local bin="$1" path
  for path in $(et_rpaths "$bin"); do
    case "$path" in
      /*) install_name_tool -delete_rpath "$path" "$bin" ;;
    esac
  done
}

et_copy_plugin() {
  local qt_root="$1" dest="$2" qt_lib="$3" sub="$4" name="$5"
  local src="${qt_root}/plugins/${sub}/${name}"
  if [[ ! -f "$src" ]]; then
    return 0
  fi
  mkdir -p "${dest}/Contents/PlugIns/${sub}"
  cp "$src" "${dest}/Contents/PlugIns/${sub}/${name}"
  et_collect_frameworks "${dest}/Contents/PlugIns/${sub}/${name}" "$qt_lib" "${dest}/Contents/Frameworks"
}

et_deploy_app() {
  local bin="$1" qt_root="$2" dest="$3"
  local qt_lib="${qt_root}/lib"
  local plist root
  root="$(et_root)"
  plist="${root}/src-qt/macos/Info.plist"
  rm -rf "$dest"
  mkdir -p "${dest}/Contents/MacOS" "${dest}/Contents/Frameworks" "${dest}/Contents/Resources"
  cp "$bin" "${dest}/Contents/MacOS/et"
  chmod +x "${dest}/Contents/MacOS/et"
  cp "$plist" "${dest}/Contents/Info.plist"
  cat > "${dest}/Contents/Resources/qt.conf" <<'EOF'
[Paths]
Plugins = PlugIns
EOF
  et_collect_frameworks "${dest}/Contents/MacOS/et" "$qt_lib" "${dest}/Contents/Frameworks"
  et_copy_plugin "$qt_root" "$dest" "$qt_lib" platforms libqcocoa.dylib
  et_copy_plugin "$qt_root" "$dest" "$qt_lib" styles libqmacstyle.dylib
  find "$dest" -type f -exec chmod u+w {} +
  local f
  while IFS= read -r -d '' f; do
    if et_is_macho "$f"; then
      et_rewrite_qt_deps "$f"
      et_ensure_rpath "$f" "@executable_path/../Frameworks"
    fi
  done < <(find "$dest" -type f -print0)
  et_delete_absolute_rpaths "${dest}/Contents/MacOS/et"
}

et_copy_missing_tree() {
  local src="$1" dest="$2"
  local f rel
  if [[ ! -d "$src" ]]; then
    return 0
  fi
  mkdir -p "$dest"
  while IFS= read -r -d '' f; do
    rel="${f#"${src}/"}"
    if [[ -e "${dest}/${rel}" ]]; then
      continue
    fi
    mkdir -p "$(dirname "${dest}/${rel}")"
    if [[ -L "$f" ]]; then
      cp -R "$f" "${dest}/${rel}"
    else
      cp "$f" "${dest}/${rel}"
    fi
  done < <(find "$src" \( -type f -o -type l \) -print0)
}

et_merge_universal_app() {
  local x64="$1" arm="$2" out="$3"
  local f rel tmp
  rm -rf "$out"
  cp -R "$x64" "$out"
  et_copy_missing_tree "${arm}/Contents/Frameworks" "${out}/Contents/Frameworks"
  et_copy_missing_tree "${arm}/Contents/PlugIns" "${out}/Contents/PlugIns"
  while IFS= read -r -d '' f; do
    rel="${f#"${arm}/"}"
    if [[ -f "${x64}/${rel}" ]] && et_is_macho "$f"; then
      tmp="$(mktemp)"
      lipo -create "${x64}/${rel}" "$f" -output "$tmp"
      mv "$tmp" "${out}/${rel}"
      chmod +x "${out}/${rel}"
    fi
  done < <(find "$arm" -type f -print0)
  codesign --force --deep --sign - "$out"
}

et_make_dmg() {
  local app="$1" dmg="$2"
  local stage
  stage="$(mktemp -d)"
  cp -R "$app" "${stage}/"
  rm -f "$dmg"
  hdiutil create -volname "et" -srcfolder "$stage" -ov -format UDZO "$dmg"
  rm -rf "$stage"
}
