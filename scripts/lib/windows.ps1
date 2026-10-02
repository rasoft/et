# 被 Windows 的 bootstrap / build 脚本点源。不要直接执行。
$ErrorActionPreference = 'Stop'

$Script:EtLibDir = $PSScriptRoot
$Script:EtRoot = (Resolve-Path (Join-Path $Script:EtLibDir '..\..')).Path
if (-not (Test-Path (Join-Path $Script:EtRoot 'Cargo.toml'))) {
    $Script:EtRoot = (Resolve-Path (Join-Path $Script:EtLibDir '..')).Path
}
if (-not (Test-Path (Join-Path $Script:EtRoot 'Cargo.toml'))) {
    throw '找不到仓库根目录。'
}

function Get-EtRoot {
    return $Script:EtRoot
}

function Get-EtCache {
    $cache = Join-Path $env:LOCALAPPDATA 'et'
    if (-not (Test-Path $cache)) {
        New-Item -ItemType Directory -Path $cache | Out-Null
    }
    return $cache
}

function Get-EtVsWhere {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        return $vswhere
    }
    return $null
}

function Get-EtVs2019Path {
    $vswhere = Get-EtVsWhere
    if (-not $vswhere) {
        return $null
    }
    $path = & $vswhere -version '[16.0,17.0)' -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1
    if ($path) {
        return $path.Trim()
    }
    return $null
}

function Import-EtVsEnv {
    $install = Get-EtVs2019Path
    if (-not $install) {
        throw '未找到带 MSVC v142 的 Visual Studio 2019。请先运行 scripts\bootstrap-windows.ps1。'
    }
    $vcvars = Join-Path $install 'VC\Auxiliary\Build\vcvars64.bat'
    if (-not (Test-Path $vcvars)) {
        throw "未找到 $vcvars"
    }
    $lines = cmd /c "call `"$vcvars`" && set"
    foreach ($line in $lines) {
        if ($line -match '^([A-Za-z_][A-Za-z0-9_]*)=(.*)$') {
            Set-Item -Path "Env:$($Matches[1])" -Value $Matches[2]
        }
    }
    if (-not (Get-Command link.exe -ErrorAction SilentlyContinue)) {
        throw 'vcvars64 之后仍然找不到 link.exe。'
    }
}

function Add-EtToolPath {
    $venvScripts = Join-Path (Get-EtCache) 'venv\Scripts'
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    $env:PATH = "$venvScripts;$cargoBin;$env:PATH"
}

function Assert-EtRustc {
    Push-Location (Get-EtRoot)
    try {
        $ver = & rustc --version
        if ($LASTEXITCODE -ne 0) {
            throw 'rustc 执行失败。'
        }
        if ($ver -notlike 'rustc 1.77.*') {
            throw "需要 Rust 1.77，当前是：$ver。请在仓库根目录构建，让 rustup 按 rust-toolchain.toml 选择 1.77.2。"
        }
    } finally {
        Pop-Location
    }
}

function Install-EtVisualStudio {
    if (Get-EtVs2019Path) {
        Write-Host '已安装 Visual Studio 2019（MSVC v142）。'
        return
    }
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        throw '未找到 Visual Studio 2019，也没有 winget。请安装 VS 2019 Build Tools，并勾选“使用 C++ 的桌面开发”和 Windows 10 SDK。'
    }
    Write-Host '安装 Visual Studio 2019 Build Tools。这一步会下载数 GB，需要管理员权限。'
    $override = '--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows10SDK.19041 --includeRecommended'
    & winget install --id Microsoft.VisualStudio.2019.BuildTools --exact --accept-package-agreements --accept-source-agreements --override $override
    if ($LASTEXITCODE -ne 0) {
        throw "winget 安装 Visual Studio 2019 失败，退出码 $LASTEXITCODE。"
    }
    if (-not (Get-EtVs2019Path)) {
        throw 'Visual Studio 2019 安装后仍未找到 MSVC v142。'
    }
}

function Get-EtSystemPython {
    foreach ($name in @('py', 'python', 'python3')) {
        $cmd = Get-Command $name -ErrorAction SilentlyContinue
        if ($cmd) {
            return $cmd
        }
    }
    return $null
}

function Install-EtPython {
    if (Get-EtSystemPython) {
        return
    }
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        throw '未找到 Python 3，也没有 winget。'
    }
    Write-Host '安装 Python 3'
    & winget install --id Python.Python.3.12 --exact --accept-package-agreements --accept-source-agreements
    if ($LASTEXITCODE -ne 0) {
        throw "winget 安装 Python 失败，退出码 $LASTEXITCODE。"
    }
    $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $user = [Environment]::GetEnvironmentVariable('Path', 'User')
    $env:PATH = "$user;$machine;$env:PATH"
}

function Install-EtVenv {
    Install-EtPython
    $venv = Join-Path (Get-EtCache) 'venv'
    $python = Join-Path $venv 'Scripts\python.exe'
    if (-not (Test-Path $python)) {
        $system = Get-EtSystemPython
        if (-not $system) {
            throw '未找到 Python 3。'
        }
        if ($system.Name -eq 'py.exe') {
            & py -3 -m venv $venv
        } else {
            & $system.Source -m venv $venv
        }
        if ($LASTEXITCODE -ne 0) {
            throw "创建 Python 虚拟环境失败，退出码 $LASTEXITCODE。"
        }
    }
    $aqt = Join-Path $venv 'Scripts\aqt.exe'
    $cmake = Join-Path $venv 'Scripts\cmake.exe'
    $ninja = Join-Path $venv 'Scripts\ninja.exe'
    if ((-not (Test-Path $aqt)) -or (-not (Test-Path $cmake)) -or (-not (Test-Path $ninja))) {
        & $python -m pip install --upgrade pip
        if ($LASTEXITCODE -ne 0) { throw 'pip 升级失败。' }
        & $python -m pip install 'aqtinstall>=3.1,<4' cmake ninja
        if ($LASTEXITCODE -ne 0) { throw '安装 aqtinstall、cmake、ninja 失败。' }
    }
    # 避免 urllib3 2 在旧 SSL 上把 Qt 包下坏。
    & $python -c "import urllib3,sys; sys.exit(0 if int(urllib3.__version__.split('.')[0]) < 2 else 1)"
    if ($LASTEXITCODE -ne 0) {
        & $python -m pip install 'urllib3<2'
        if ($LASTEXITCODE -ne 0) { throw '安装 urllib3<2 失败。' }
    }
}

function Install-EtQt {
    $cache = Get-EtCache
    $qtRoot = Join-Path $cache 'qt\5.15.2\msvc2019_64'
    $config = Join-Path $qtRoot 'lib\cmake\Qt5\Qt5Config.cmake'
    if (-not (Test-Path $config)) {
        Write-Host '下载 Qt 5.15.2 msvc2019_64（只要 qtbase）'
        $aqt = Join-Path $cache 'venv\Scripts\aqt.exe'
        $aqtArgs = @(
            'install-qt', 'windows', 'desktop', '5.15.2', 'win64_msvc2019_64',
            '--outputdir', (Join-Path $cache 'qt'),
            '--archives', 'qtbase', 'd3dcompiler_47', 'opengl32sw',
            '--timeout', '300'
        )
        if ($env:ET_QT_MIRROR) {
            $aqtArgs += @('-b', $env:ET_QT_MIRROR)
        }
        $downloaded = $false
        foreach ($attempt in 1..3) {
            Push-Location $cache
            try {
                & $aqt @aqtArgs
                $code = $LASTEXITCODE
            } finally {
                Pop-Location
            }
            if ($code -eq 0) {
                $downloaded = $true
                break
            }
            Write-Host "Qt 下载失败，重试 $attempt/3。若镜像一直超时，可设置 ET_QT_MIRROR 后重跑。"
            Start-Sleep -Seconds 2
        }
        if (-not $downloaded) {
            throw 'aqt 安装 Qt 失败。'
        }
    }
    if (-not (Test-Path $config)) {
        throw "Qt 已下载，但没找到 $config。"
    }
    Set-Content -Path (Join-Path $cache 'qt-root') -Value $qtRoot -Encoding ascii
    return $qtRoot
}

function Install-EtRust {
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    $rustup = Join-Path $cargoBin 'rustup.exe'
    if (-not (Test-Path $rustup)) {
        Write-Host '安装 rustup，并带上 Rust 1.77.2'
        $init = Join-Path (Get-EtCache) 'rustup-init.exe'
        Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile $init
        & $init -y --default-toolchain 1.77.2 --profile minimal --component rustfmt,clippy
        if ($LASTEXITCODE -ne 0) {
            throw "rustup-init 失败，退出码 $LASTEXITCODE。"
        }
    }
    $env:PATH = "$cargoBin;$env:PATH"
    & rustup toolchain install 1.77.2 --profile minimal --component rustfmt,clippy
    if ($LASTEXITCODE -ne 0) { throw '安装 Rust 1.77.2 失败。' }
    & rustup target add x86_64-pc-windows-msvc --toolchain 1.77.2
    if ($LASTEXITCODE -ne 0) { throw '添加 Windows MSVC 目标失败。' }
}
