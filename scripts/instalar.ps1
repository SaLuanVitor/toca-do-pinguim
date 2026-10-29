# Instala a versão mais recente da Toca do Pinguim para o usuário atual, sem pedir
# administrador. Confere o SHA256 do instalador antes de rodá-lo.
#
#   irm https://raw.githubusercontent.com/SaLuanVitor/toca-do-pinguim/main/scripts/instalar.ps1 | iex

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$repo = 'SaLuanVitor/toca-do-pinguim'

$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$setup = $release.assets | Where-Object name -like '*_x64-setup.exe' | Select-Object -First 1
$somas = $release.assets | Where-Object name -eq 'SHA256SUMS.txt' | Select-Object -First 1
if (-not $setup -or -not $somas) { throw "A release $($release.tag_name) não traz o instalador e o SHA256SUMS.txt." }

$pasta = Join-Path ([IO.Path]::GetTempPath()) "toca-do-pinguim-$([guid]::NewGuid().ToString('N').Substring(0, 8))"
New-Item -ItemType Directory $pasta | Out-Null
try {
  $destino = Join-Path $pasta $setup.name
  Write-Host "Baixando $($setup.name) ($($release.tag_name))"
  Invoke-WebRequest $setup.browser_download_url -OutFile $destino -UseBasicParsing
  Invoke-WebRequest $somas.browser_download_url -OutFile (Join-Path $pasta 'SHA256SUMS.txt') -UseBasicParsing

  $linha = Get-Content (Join-Path $pasta 'SHA256SUMS.txt') | Where-Object { $_ -match "\s$([regex]::Escape($setup.name))$" } | Select-Object -First 1
  $esperado = ($linha -split '\s+')[0]
  $obtido = (Get-FileHash $destino -Algorithm SHA256).Hash.ToLower()
  if (-not $esperado -or $obtido -ne $esperado) { throw "SHA256 não confere: esperado '$esperado', obtido '$obtido'. Nada foi instalado." }

  $p = Start-Process $destino -ArgumentList '/S' -PassThru -Wait
  if ($p.ExitCode -ne 0) { throw "O instalador saiu com o código $($p.ExitCode)." }
  Write-Host "Toca do Pinguim $($release.tag_name) instalada. Abra pelo menu Iniciar, pelo Win+R ou com: start toca-do-pinguim"
} finally {
  Remove-Item $pasta -Recurse -Force -ErrorAction SilentlyContinue
}
