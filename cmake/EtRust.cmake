find_program(CARGO cargo
  HINTS "$ENV{HOME}/.cargo/bin" "$ENV{USERPROFILE}/.cargo/bin"
  REQUIRED)

if(WIN32)
  find_program(ET_MSVC_LINK link)
  if(NOT ET_MSVC_LINK)
    message(FATAL_ERROR
      "未找到 link.exe。请在已经执行 vcvars64.bat 的环境里配置，或运行 scripts/build-windows.ps1。")
  endif()
  set(ET_RUST_TARGET "x86_64-pc-windows-msvc")
elseif(APPLE)
  if(CMAKE_OSX_ARCHITECTURES MATCHES ";")
    message(FATAL_ERROR "暂不构建 Universal Binary。x86_64 与 arm64 请分开配置。")
  elseif(CMAKE_OSX_ARCHITECTURES STREQUAL "arm64")
    set(ET_RUST_TARGET "aarch64-apple-darwin")
  else()
    set(ET_RUST_TARGET "x86_64-apple-darwin")
  endif()
else()
  set(ET_RUST_TARGET "")
endif()

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
  set(ET_CARGO_PROFILE_DIR "debug")
  set(ET_CARGO_RELEASE FALSE)
else()
  set(ET_CARGO_PROFILE_DIR "release")
  set(ET_CARGO_RELEASE TRUE)
endif()

set(ET_CARGO_TARGET_DIR "${CMAKE_BINARY_DIR}/cargo")
if(ET_RUST_TARGET)
  set(ET_CARGO_LIB_DIR "${ET_CARGO_TARGET_DIR}/${ET_RUST_TARGET}/${ET_CARGO_PROFILE_DIR}")
else()
  set(ET_CARGO_LIB_DIR "${ET_CARGO_TARGET_DIR}/${ET_CARGO_PROFILE_DIR}")
endif()

set(ET_CMD_LIB "${ET_CARGO_LIB_DIR}/${CMAKE_STATIC_LIBRARY_PREFIX}et_cmd${CMAKE_STATIC_LIBRARY_SUFFIX}")

set(ET_CARGO_ENV_ARGS
  "CARGO_TARGET_DIR=${ET_CARGO_TARGET_DIR}"
  "CARGO_TERM_COLOR=never")
if(APPLE)
  if(NOT CMAKE_OSX_DEPLOYMENT_TARGET)
    message(FATAL_ERROR "macOS 构建必须设置 CMAKE_OSX_DEPLOYMENT_TARGET。")
  endif()
  list(APPEND ET_CARGO_ENV_ARGS "MACOSX_DEPLOYMENT_TARGET=${CMAKE_OSX_DEPLOYMENT_TARGET}")
endif()

set(ET_CARGO_BUILD_ARGS
  --manifest-path "${CMAKE_SOURCE_DIR}/Cargo.toml"
  -p et-cmd
  --locked)
if(ET_RUST_TARGET)
  list(APPEND ET_CARGO_BUILD_ARGS --target "${ET_RUST_TARGET}")
endif()
if(ET_CARGO_RELEASE)
  list(APPEND ET_CARGO_BUILD_ARGS --release)
endif()

message(STATUS "Probing Rust native libraries (${ET_CARGO_PROFILE_DIR})")
execute_process(
  COMMAND ${CMAKE_COMMAND} -E env ${ET_CARGO_ENV_ARGS}
    ${CARGO} rustc ${ET_CARGO_BUILD_ARGS} -- --print native-static-libs
  WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
  RESULT_VARIABLE ET_CARGO_STATUS
  OUTPUT_VARIABLE ET_CARGO_STDOUT
  ERROR_VARIABLE ET_CARGO_STDERR
  OUTPUT_STRIP_TRAILING_WHITESPACE
  ERROR_STRIP_TRAILING_WHITESPACE)

if(NOT ET_CARGO_STATUS EQUAL 0)
  message(FATAL_ERROR "cargo rustc 失败（${ET_CARGO_STATUS}）。\n${ET_CARGO_STDERR}\n${ET_CARGO_STDOUT}")
endif()
if(NOT EXISTS "${ET_CMD_LIB}")
  message(FATAL_ERROR "cargo 已结束，但没找到 ${ET_CMD_LIB}")
endif()

string(REGEX MATCH "native-static-libs: ([^\r\n]+)" ET_NATIVE_MATCH "${ET_CARGO_STDERR}")
if(NOT ET_NATIVE_MATCH)
  message(FATAL_ERROR "没能从 rustc 输出里解析 native-static-libs。\n${ET_CARGO_STDERR}")
endif()

string(STRIP "${CMAKE_MATCH_1}" ET_NATIVE_LINE)
string(REGEX REPLACE "[ \t]+" ";" ET_NATIVE_RAW "${ET_NATIVE_LINE}")

set(ET_RUST_NATIVE_LIBS "")
set(_et_pending_framework "")
foreach(_item IN LISTS ET_NATIVE_RAW)
  if(_et_pending_framework)
    list(APPEND ET_RUST_NATIVE_LIBS "-Wl,${_et_pending_framework},${_item}")
    set(_et_pending_framework "")
  elseif(_item STREQUAL "-framework" OR _item STREQUAL "-weak_framework")
    set(_et_pending_framework "${_item}")
  else()
    list(APPEND ET_RUST_NATIVE_LIBS "${_item}")
  endif()
endforeach()
if(_et_pending_framework)
  message(FATAL_ERROR "native-static-libs 里的 ${_et_pending_framework} 缺少框架名：${ET_NATIVE_LINE}")
endif()

message(STATUS "Rust native libs: ${ET_RUST_NATIVE_LIBS}")

file(GLOB_RECURSE ET_RUST_SOURCES CONFIGURE_DEPENDS
  "${CMAKE_SOURCE_DIR}/crates/*.rs"
  "${CMAKE_SOURCE_DIR}/crates/*.toml")
list(APPEND ET_RUST_SOURCES
  "${CMAKE_SOURCE_DIR}/Cargo.toml"
  "${CMAKE_SOURCE_DIR}/Cargo.lock"
  "${CMAKE_SOURCE_DIR}/rust-toolchain.toml"
  "${CMAKE_SOURCE_DIR}/.cargo/config.toml")

add_custom_command(
  OUTPUT "${ET_CMD_LIB}"
  COMMAND ${CMAKE_COMMAND} -E env ${ET_CARGO_ENV_ARGS}
    ${CARGO} build ${ET_CARGO_BUILD_ARGS}
  WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
  DEPENDS ${ET_RUST_SOURCES}
  COMMENT "Building et-cmd (${ET_CARGO_PROFILE_DIR})"
  USES_TERMINAL
  VERBATIM)

add_custom_target(et_cmd_cargo DEPENDS "${ET_CMD_LIB}")

add_library(et_cmd STATIC IMPORTED GLOBAL)
set_target_properties(et_cmd PROPERTIES IMPORTED_LOCATION "${ET_CMD_LIB}")
add_dependencies(et_cmd et_cmd_cargo)
