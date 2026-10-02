# 编译 Windows x64 的 et。用法：
#   powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1 [Debug|Release]
#Requires -Version 5.1
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot 'lib\windows.ps1')

$buildType = 'Release'
if ($args.Count -ge 1) {
    $buildType = $args[0]
}
if ($buildType -ne 'Debug' -and $buildType -ne 'Release') {
    throw '构建类型只能是 Debug 或 Release。'
}
if ($buildType -eq 'Debug') {
    Write-Warning 'aqt 安装的 Qt 5.15.2 只有 Release 库，Debug 可能无法链接。'
}

Import-EtVsEnv
Add-EtToolPath

$root = Get-EtRoot
Set-Location $root
Assert-EtRustc

foreach ($tool in @('cmake', 'ninja', 'cargo')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "缺少 $tool。请先运行 scripts\bootstrap-windows.ps1。"
    }
}

$qtRoot = $env:ET_QT_ROOT
if (-not $qtRoot) {
    $qtRootFile = Join-Path (Get-EtCache) 'qt-root'
    if (-not (Test-Path $qtRootFile)) {
        throw '未找到 Qt。请先运行 scripts\bootstrap-windows.ps1，或设置 ET_QT_ROOT。'
    }
    $qtRoot = (Get-Content $qtRootFile -Raw).Trim()
}
$config = Join-Path $qtRoot 'lib\cmake\Qt5\Qt5Config.cmake'
if (-not (Test-Path $config)) {
    throw "没找到 Qt5Config.cmake：$qtRoot"
}

if (-not (Test-Path (Join-Path $root 'Cargo.lock'))) {
    & cargo generate-lockfile
    if ($LASTEXITCODE -ne 0) { throw 'cargo generate-lockfile 失败。' }
}

& cargo test --locked --workspace
if ($LASTEXITCODE -ne 0) { throw 'cargo test 失败。' }

$buildDir = Join-Path $root 'build\windows'
& cmake -G Ninja -S $root -B $buildDir "-DCMAKE_BUILD_TYPE=$buildType" "-DCMAKE_PREFIX_PATH=$qtRoot"
if ($LASTEXITCODE -ne 0) { throw 'cmake 配置失败。' }
& cmake --build $buildDir
if ($LASTEXITCODE -ne 0) { throw '编译失败。' }

$compileCommands = Join-Path $buildDir 'compile_commands.json'
$link = Join-Path $root 'compile_commands.json'
if (Test-Path $compileCommands) {
    try {
        if (Test-Path $link) { Remove-Item $link -Force }
        New-Item -ItemType SymbolicLink -Path $link -Target $compileCommands -ErrorAction Stop | Out-Null
    } catch {
        Write-Warning "没能创建 compile_commands.json 链接：$($_.Exception.Message)"
    }
}

Write-Host "已生成：$(Join-Path $buildDir 'et.exe')"
