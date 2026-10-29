//! Tecla de atalho do Windows para abrir o app. Ela mora no atalho do menu Iniciar (o
//! mesmo `.lnk` que o instalador cria), e é o próprio Windows que a escuta: funciona com
//! o app fechado, sem nada rodando em segundo plano. Com o app aberto, a instância única
//! traz a janela para frente.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use log::info;
use serde::Serialize;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `$SMPROGRAMS\${PRODUCTNAME}.lnk` do instalador NSIS do Tauri, na instalação por usuário.
fn caminho_do_atalho() -> PathBuf {
    let roaming = std::env::var_os("APPDATA").unwrap_or_default();
    PathBuf::from(roaming).join(r"Microsoft\Windows\Start Menu\Programs\Toca do Pinguim.lnk")
}

/// O `.lnk` é editado pelo `WScript.Shell`, pela interface COM que o Windows já traz. Os
/// valores vão por variável de ambiente, não colados no script, para nenhum caractere do
/// caminho virar código do PowerShell.
fn powershell(script: &str, variaveis: &[(&str, &str)]) -> Result<String, String> {
    let mut c = Command::new("powershell.exe");
    c.creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script]);
    for (k, v) in variaveis {
        c.env(k, v);
    }
    let saida = c.output().map_err(|e| format!("powershell.exe: {e}"))?;
    if saida.status.success() {
        Ok(String::from_utf8_lossy(&saida.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&saida.stderr).trim().to_string())
    }
}

#[derive(Serialize)]
pub struct AtalhoTeclado {
    /// No formato do Windows, como `Ctrl+Alt+P`; vazio sem tecla.
    tecla: String,
    caminho: String,
    existe: bool,
}

#[tauri::command]
pub fn ler_atalho_teclado() -> Result<AtalhoTeclado, String> {
    let caminho = caminho_do_atalho();
    let existe = caminho.is_file();
    let tecla = if existe {
        powershell(
            "(New-Object -ComObject WScript.Shell).CreateShortcut($env:TOCA_LNK).Hotkey",
            &[("TOCA_LNK", &caminho.to_string_lossy())],
        )?
    } else {
        String::new()
    };
    Ok(AtalhoTeclado { tecla, caminho: caminho.display().to_string(), existe })
}

/// Grava a tecla (vazia remove). Sem atalho no menu Iniciar, como na versão portátil, cria
/// um apontando para este executável; o de uma instalação continua apontando para ela.
#[tauri::command]
pub fn definir_atalho_teclado(tecla: String) -> Result<(), String> {
    if !tecla.chars().all(|c| c.is_ascii_alphanumeric() || c == '+') {
        return Err(format!("tecla inválida: {tecla}"));
    }
    let caminho = caminho_do_atalho();
    let alvo = std::env::current_exe().map_err(|e| e.to_string())?;
    let script = r#"
        $s = (New-Object -ComObject WScript.Shell).CreateShortcut($env:TOCA_LNK)
        if ($env:TOCA_CRIAR -eq '1') {
            $s.TargetPath = $env:TOCA_ALVO
            $s.WorkingDirectory = Split-Path -Parent $env:TOCA_ALVO
            $s.Description = 'Gerenciador do WSL'
        }
        # Regravar o atalho deixa o ícone como ",0", sem caminho, e o menu Iniciar mostra um
        # ícone em branco: aponta sempre para o executável do próprio atalho.
        if (-not $s.IconLocation -or $s.IconLocation.StartsWith(',')) { $s.IconLocation = "$($s.TargetPath),0" }
        $s.Hotkey = $env:TOCA_TECLA
        $s.Save()
    "#;
    let criar = if caminho.is_file() { "0" } else { "1" };
    powershell(
        script,
        &[
            ("TOCA_LNK", &caminho.to_string_lossy()),
            ("TOCA_ALVO", &alvo.to_string_lossy()),
            ("TOCA_CRIAR", criar),
            ("TOCA_TECLA", &tecla),
        ],
    )?;
    info!("tecla de atalho: [{tecla}] em {} (criado agora: {})", caminho.display(), criar == "1");
    Ok(())
}
