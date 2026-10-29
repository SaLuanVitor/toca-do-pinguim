# Remove a Toca do Pinguim instalada pelo instalador. Fecha o app antes, se estiver aberto.
# Os dados (histórico de memória, atividade e log) ficam, a não ser com -ApagarDados.
#
#   irm https://raw.githubusercontent.com/SaLuanVitor/toca-do-pinguim/main/scripts/desinstalar.ps1 | iex
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/SaLuanVitor/toca-do-pinguim/main/scripts/desinstalar.ps1))) -ApagarDados

param([switch]$ApagarDados)

$ErrorActionPreference = 'Stop'
$chave = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Toca do Pinguim'

if (Test-Path $chave) {
  $desinstalador = (Get-ItemProperty $chave).UninstallString.Trim('"')
  Get-Process toca-do-pinguim -ErrorAction SilentlyContinue | ForEach-Object {
    $_.CloseMainWindow() | Out-Null
    if (-not $_.WaitForExit(15000)) { $_.Kill() }
  }
  $p = Start-Process $desinstalador -ArgumentList '/S' -PassThru -Wait
  if ($p.ExitCode -ne 0) { throw "O desinstalador saiu com o código $($p.ExitCode)." }
  Write-Host 'Toca do Pinguim removida.'
} else {
  Write-Host 'A Toca do Pinguim não está instalada por instalador.'
}

if ($ApagarDados) {
  foreach ($pasta in "$env:APPDATA\com.luan.tocadopinguim", "$env:LOCALAPPDATA\com.luan.tocadopinguim") {
    if (Test-Path $pasta) { Remove-Item $pasta -Recurse -Force; Write-Host "Apagado: $pasta" }
  }
}
