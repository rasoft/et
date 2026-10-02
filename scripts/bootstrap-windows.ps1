# 建立 Windows 构建环境：Visual Studio 2019、Rust 1.77.2、CMake、Ninja、Qt 5.15.2。
# 构建机是 64 位 Windows。产物的子系统版本是 Windows 7，不在 Windows 7 上安装这套工具。
# 可重复执行。用法：
#   powershell -ExecutionPolicy Bypass -File scripts\bootstrap-windows.ps1
#Requires -Version 5.1
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot 'lib\windows.ps1')

if (-not [Environment]::Is64BitOperatingSystem) {
    throw '只支持 64 位 Windows 构建机。'
}

Write-Host '构建机可以是更新的 64 位 Windows。可执行文件子系统版本固定为 Windows 7。'

Install-EtVisualStudio
Install-EtVenv
Install-EtQt
Install-EtRust

$qtRoot = (Get-Content (Join-Path (Get-EtCache) 'qt-root') -Raw).Trim()
Write-Host 'Windows 构建环境已就绪。'
Write-Host "Qt：$qtRoot"
Write-Host '下一步：powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1'
