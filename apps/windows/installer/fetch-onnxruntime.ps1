<#
.SYNOPSIS
    取知意（ONNX）及其 Visual C++ 运行库到 target\installer\onnxruntime\，与程序同目录部署。
.DESCRIPTION
    ONNX Runtime 1.24.4 取自 NuGet 的 Microsoft.ML.OnnxRuntime（CPU 版），按包的 SHA-256 核对，已取过就跳过。
    同时取 Microsoft VC Runtime 14.44.35211，避免依赖用户预先安装 VC Redistributable。
    Server 开发时也从这里加载（见 apps\windows\server\src\dispatch\rescore\models.rs），装机时 qingjian.iss 把它们装到 exe 旁边。
#>
$ErrorActionPreference = 'Stop'
# 进度条会让 Invoke-WebRequest 慢好几倍
$ProgressPreference = 'SilentlyContinue'

$Version = '1.24.4'
# 换了包（版本或 CPU / GPU 版）就改这个，旧的缓存才会重取
$Stamp_Value = "$Version-cpu-vc14.44.35211"
$CrtPath = 'Contents/VC/Redist/MSVC/14.44.35112/x64/Microsoft.VC143.CRT'
$Packages = @(
    @{
        Url    = "https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/$Version/microsoft.ml.onnxruntime.$Version.nupkg"
        Sha256 = '4b978d5065b85e7004b6c6f60ca494bd978fbe6836cbf0a0b52d82b61ab99638'
        Files  = @{
            'runtimes/win-x64/native/onnxruntime.dll' = 'onnxruntime.dll'
            'runtimes/win-x64/native/onnxruntime_providers_shared.dll' = 'onnxruntime_providers_shared.dll'
            'LICENSE'                                 = 'onnxruntime-LICENSE.txt'
            'ThirdPartyNotices.txt'                   = 'onnxruntime-ThirdPartyNotices.txt'
        }
    },
    @{
        # 官方 Visual Studio 17.14 清单中的可分发 CRT，仅取发行版目录，不取 debug_nonredist。
        Url = 'https://download.visualstudio.microsoft.com/download/pr/45d3b8dd-bced-4b37-9974-142f748d710c/4aaf54db0bfc9435f7c3660e1a00237a4b556042bfeea64bde44c2e0194e6ee5/Microsoft.VC.14.44.17.14.CRT.Redist.X64.base.vsix'
        Sha256 = '4aaf54db0bfc9435f7c3660e1a00237a4b556042bfeea64bde44c2e0194e6ee5'
        Files = @{
            "$CrtPath/msvcp140.dll" = 'msvcp140.dll'
            "$CrtPath/msvcp140_1.dll" = 'msvcp140_1.dll'
            "$CrtPath/msvcp140_2.dll" = 'msvcp140_2.dll'
            "$CrtPath/msvcp140_atomic_wait.dll" = 'msvcp140_atomic_wait.dll'
            "$CrtPath/msvcp140_codecvt_ids.dll" = 'msvcp140_codecvt_ids.dll'
            "$CrtPath/vcruntime140.dll" = 'vcruntime140.dll'
            "$CrtPath/vcruntime140_1.dll" = 'vcruntime140_1.dll'
            "$CrtPath/vcruntime140_threads.dll" = 'vcruntime140_threads.dll'
            "$CrtPath/concrt140.dll" = 'concrt140.dll'
            "$CrtPath/vccorlib140.dll" = 'vccorlib140.dll'
        }
    }
)

$Repo  = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$Dest  = Join-Path $Repo 'target\installer\onnxruntime'
$Stamp = Join-Path $Dest 'VERSION'
if ((Test-Path $Stamp) -and ((Get-Content $Stamp -Raw).Trim() -eq $Stamp_Value)) {
    Write-Host "ONNX Runtime $Version 已在 $Dest" -ForegroundColor Cyan
    return
}
if (Test-Path $Dest) { Remove-Item $Dest -Recurse -Force }
New-Item -ItemType Directory -Path $Dest -Force | Out-Null

Add-Type -AssemblyName System.IO.Compression.FileSystem
foreach ($package in $Packages) {
    $download = Join-Path $env:TEMP ([IO.Path]::GetFileName($package.Url))
    Write-Host "下载 $($package.Url)" -ForegroundColor Cyan
    Invoke-WebRequest -Uri $package.Url -OutFile $download -UseBasicParsing
    $hash = (Get-FileHash $download -Algorithm SHA256).Hash.ToLower()
    if ($hash -ne $package.Sha256) {
        Remove-Item $download -Force
        throw "$download 的 SHA-256 是 $hash，应为 $($package.Sha256)"
    }
    $zip = [IO.Compression.ZipFile]::OpenRead($download)
    try {
        foreach ($entry in $package.Files.Keys) {
            $found = $zip.Entries | Where-Object { $_.FullName -eq $entry } | Select-Object -First 1
            if (-not $found) { throw "$download 里没有 $entry" }
            [IO.Compression.ZipFileExtensions]::ExtractToFile($found, (Join-Path $Dest $package.Files[$entry]), $true)
        }
    } finally {
        $zip.Dispose()
        Remove-Item $download -Force
    }
}
Set-Content -Path $Stamp -Value $Stamp_Value -Encoding ASCII
Write-Host "ONNX Runtime $Version → $Dest" -ForegroundColor Green
