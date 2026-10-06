cmake_minimum_required(VERSION 3.16)

# CMake 4 不再接受 Qt 5.15 自带脚本里的旧 cmake_minimum_required。
if(NOT DEFINED CMAKE_POLICY_VERSION_MINIMUM)
  set(CMAKE_POLICY_VERSION_MINIMUM 3.5)
endif()

# 必须写在 project() 之前，否则 macOS 会按本机架构和当前系统版本配置。
if(CMAKE_HOST_SYSTEM_NAME STREQUAL "Darwin")
  if(NOT CMAKE_OSX_DEPLOYMENT_TARGET)
    set(CMAKE_OSX_DEPLOYMENT_TARGET "10.13" CACHE STRING
      "Qt 5.15 supports macOS 10.13 and later")
  endif()
  if(NOT CMAKE_OSX_ARCHITECTURES)
    set(CMAKE_OSX_ARCHITECTURES "x86_64" CACHE STRING
      "Single-arch configure. scripts/build-macos.sh builds x86_64 and arm64, then lipos them.")
  endif()
endif()

project(et VERSION 0.1.0 LANGUAGES CXX)

if(CMAKE_CONFIGURATION_TYPES)
  message(FATAL_ERROR
    "请使用 Ninja 或 Makefile 这类单配置生成器。Windows 由 scripts/build-windows.ps1 调用 Ninja。")
endif()

if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release CACHE STRING "Debug or Release")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

find_package(Qt5 5.15 REQUIRED COMPONENTS Widgets)

include(cmake/EtRust.cmake)

add_executable(et
  src-qt/main.cpp
  src-qt/MainWindow.h
  src-qt/MainWindow.cpp
  src-qt/ToolbarIcons.h
  src-qt/ToolbarIcons.cpp
  src-qt/ImportChoicesDialog.h
  src-qt/ImportChoicesDialog.cpp
  src-qt/EtSession.h
  src-qt/EtSession.cpp
  src-qt/PartitionListWidget.h
  src-qt/PartitionListWidget.cpp
  src-qt/PartitionEditorWidget.h
  src-qt/PartitionEditorWidget.cpp
  src-qt/et.qrc)
add_dependencies(et et_cmd_cargo)
target_include_directories(et PRIVATE "${CMAKE_SOURCE_DIR}/crates/et-cmd/include")
target_link_libraries(et PRIVATE et_cmd ${ET_RUST_NATIVE_LIBS} Qt5::Widgets)

if(MSVC)
  target_compile_definitions(et PRIVATE NOMINMAX _WIN32_WINNT=0x0601 WINVER=0x0601)
  # 不链接 Qt 的 WinMain。入口仍是 main，子系统版本固定为 Windows 7。
  set_target_properties(et PROPERTIES WIN32_EXECUTABLE FALSE)
  target_link_options(et PRIVATE "/SUBSYSTEM:WINDOWS,6.01" "/ENTRY:mainCRTStartup")
endif()

get_filename_component(_qt_lib_dir "${Qt5_DIR}/../.." ABSOLUTE)
if(UNIX)
  set_target_properties(et PROPERTIES
    BUILD_RPATH "${_qt_lib_dir}"
    INSTALL_RPATH "${_qt_lib_dir}")
endif()

if(WIN32)
  set(ET_ICON_PATH "${CMAKE_SOURCE_DIR}/src-qt/icons/et.ico")
  string(REPLACE "\\" "/" ET_ICON_PATH "${ET_ICON_PATH}")
  configure_file(src-qt/et.rc.in "${CMAKE_BINARY_DIR}/et.rc" @ONLY)
  target_sources(et PRIVATE "${CMAKE_BINARY_DIR}/et.rc")

  get_filename_component(_qt_kit "${Qt5_DIR}/../../.." ABSOLUTE)
  find_program(ET_WINDEPLOYQT windeployqt HINTS "${_qt_kit}/bin")
  if(ET_WINDEPLOYQT)
    add_custom_command(
      TARGET et POST_BUILD
      COMMAND "${ET_WINDEPLOYQT}" --no-translations "$<TARGET_FILE:et>"
      COMMENT "Copying Qt runtime next to et.exe"
      VERBATIM)
  endif()
endif()

if(APPLE)
  message(STATUS "macOS arch: ${CMAKE_OSX_ARCHITECTURES}")
  message(STATUS "macOS deployment target: ${CMAKE_OSX_DEPLOYMENT_TARGET}")
endif()
message(STATUS "Qt5: ${Qt5_DIR}")
