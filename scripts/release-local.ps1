# The custom fork's release made on this PC instead of by GitHub Actions
# (.github/workflows/custom-release.yml does the same on GitHub): builds the Windows x64 zip,
# zips the traffic add-ons, tags the commit v<MAJOR.MINOR.COMMIT>-custom and publishes the
# release with its notes. Needs what build-windows.cmd needs, plus the GitHub CLI signed in
# to the fork (winget install GitHub.cli; gh auth login).
#   powershell -ExecutionPolicy Bypass -File scripts\release-local.ps1
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw "Install the GitHub CLI (winget install GitHub.cli), run 'gh auth login', then run this again."
}
if (git status --porcelain --untracked-files=no) { throw "Commit or stash your changes first." }
$branch = git rev-parse --abbrev-ref HEAD
if ($branch -ne "custom") { throw "Check out the custom branch first (now on $branch)." }
git push origin custom
if ($LASTEXITCODE) { throw "git push failed." }

# (as scripts/version.sh)
$base = (Get-Content VERSION -Raw).Trim()
$last = git log -1 --format=%H -- VERSION
$n = if ($last) { git rev-list --count --invert-grep --fixed-strings --grep="[skip ci]" --grep="[skip actions]" "$last..HEAD" } else { 0 }
$version = "$base.$n-custom"
$tag = "v$version"
Write-Host "== openOMSI $version" -ForegroundColor Cyan

$env:OPENOMSI_VERSION = $version
cmd /c scripts\build-windows.cmd
if ($LASTEXITCODE) { throw "The build failed." }

# DirectX 12's shader compiler and the Visual C++ runtime beside the program
try {
    Remove-Item -Recurse -Force dxc -ErrorAction SilentlyContinue
    gh release download -R microsoft/DirectXShaderCompiler --pattern "dxc_*.zip" -D dxc
    $zip = Get-ChildItem dxc/*.zip | Where-Object { $_.Name -notmatch "linux|pdb" } | Select-Object -First 1
    Expand-Archive $zip.FullName -DestinationPath dxc/x
    Copy-Item "dxc/x/bin/x64/dxcompiler.dll", "dxc/x/bin/x64/dxil.dll" dist/windows/
} catch { Write-Warning "No DirectX shader compiler: $_" }
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$crt = $null
if (Test-Path $vswhere) {
    $vs = & $vswhere -latest -products * -property installationPath
    $crt = Get-ChildItem "$vs\VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT" -Directory -ErrorAction SilentlyContinue | Sort-Object FullName | Select-Object -Last 1
}
$dir = if ($crt) { $crt.FullName } else { "$env:SystemRoot\System32" }
foreach ($dll in "vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll") { Copy-Item (Join-Path $dir $dll) dist/windows/ }

Remove-Item -Recurse -Force out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force out | Out-Null
Copy-Item README.md, LICENSE dist/windows/
Compress-Archive -Path dist/windows/* -DestinationPath "out/openOMSI-$version-windows-x64.zip"
Compress-Archive -Path addons/TH_Wald/* -DestinationPath "out/TH_Wald-traffic-addon.zip"
Compress-Archive -Path "addons/Budapest 181/*" -DestinationPath "out/Budapest181-traffic-addon.zip"
Get-ChildItem out

# The notes (as the workflow's)
git fetch --no-tags -q https://github.com/openOMSI-Project/openOMSI.git main
$upstream = git rev-parse FETCH_HEAD
git fetch --tags -q origin
$prev = git tag -l "v*-custom" --sort=-creatordate | Where-Object { $_ -ne $tag } | Select-Object -First 1
$repo = "https://github.com/rizzicusnimrod/openOMSI"
$notes = @(
    "**openOMSI $version** - a custom fork of [openOMSI](https://github.com/openOMSI-Project/openOMSI).",
    "",
    "### Install",
    "",
    "You need **OMSI 2** (Steam) installed: openOMSI uses its maps, buses and sounds.",
    "",
    "1. Download ``openOMSI-$version-windows-x64.zip``, unzip it into a folder of its own and start ``openomsi.exe``.",
    "2. Optional: ``TH_Wald-traffic-addon.zip`` gives the Thüringer Wald map realistic traffic by the hour, weekday and school holidays (1995-2003). Unzip it into the OMSI 2 folder (``...\steamapps\common\OMSI 2``); it replaces ``maps\TH_Wald\unsched_trafficdens.txt`` and ``Holidays*.txt``, so back those up first if you want to go back. It also adds ``maps\TH_Wald\retroreflective.cfg`` (which signs shine back in the headlights).",
    "3. Optional: ``Budapest181-traffic-addon.zip`` gives the Budapest 181 map traffic by the hour, weekday and school holidays (the map itself keeps it the same day and night). Unzip it into the OMSI 2 folder; it adds ``maps\Budapest 181\unsched_trafficdens.txt``.",
    "",
    "For multiplayer, everyone should run the same version. Newer versions: [the latest release]($repo/releases/latest).",
    ""
)
if ($prev) {
    $notes += "### New since $prev", ""
    $notes += git log --no-merges --format="- %s" "$prev..HEAD" "^$upstream"
    $notes += ""
}
$notes += "<details><summary>Everything this fork changes</summary>", ""
$notes += git log --no-merges --format="- %s" "$upstream..HEAD"
$notes += "", "</details>"
[IO.File]::WriteAllLines("$PWD\out\notes.md", $notes, (New-Object Text.UTF8Encoding $false))

git tag -f $tag HEAD
git push -f origin $tag
if ($LASTEXITCODE) { throw "Pushing the tag failed." }
$zips = (Get-ChildItem out/*.zip).FullName
gh release view $tag 2>$null | Out-Null
if ($LASTEXITCODE -eq 0) {
    gh release upload $tag @zips --clobber
    gh release edit $tag --title "openOMSI $version" --notes-file out/notes.md --latest
} else {
    gh release create $tag @zips --verify-tag --latest --title "openOMSI $version" --notes-file out/notes.md
}
if ($LASTEXITCODE) { throw "Publishing the release failed." }
Write-Host "== Published: $repo/releases/tag/$tag" -ForegroundColor Green
