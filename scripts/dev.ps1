# Local workspace convenience; other machines can use their normal Rust setup.
$rustBin = Join-Path $env:USERPROFILE '.cargo\bin'
$compilerBin = Join-Path $PSScriptRoot '..\.tools\gcc\mingw64\bin'
$env:PATH = "$rustBin;$compilerBin;$env:PATH"
if ($args.Count -eq 0) { & cargo run --release } else { & cargo @args }
exit $LASTEXITCODE
