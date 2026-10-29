//! Log técnico em arquivo, para diagnosticar falha do WSL depois que ela passou. Fica em
//! `%LOCALAPPDATA%\com.luan.tocadopinguim\logs`, com até 1 MB por arquivo e as 5 últimas
//! rotações. Sucessos de rotina não entram: o supervisor chama o `wsl.exe` a cada 2 s, e
//! o arquivo só guardaria isso.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

const ARQUIVO: &str = "toca-do-pinguim";

pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_log::Builder::new()
        .clear_targets()
        .target(Target::new(TargetKind::LogDir { file_name: Some(ARQUIVO.into()) }))
        .target(Target::new(TargetKind::Stdout))
        .level(log::LevelFilter::Info)
        .max_file_size(1_000_000)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build()
}

fn pasta(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_log_dir().map_err(|e| e.to_string())
}

/// As últimas `linhas` do arquivo atual, da mais antiga para a mais nova.
#[tauri::command]
pub fn ler_log(app: AppHandle, linhas: usize) -> Result<Vec<String>, String> {
    let arquivo = pasta(&app)?.join(format!("{ARQUIVO}.log"));
    let Ok(bytes) = std::fs::read(&arquivo) else { return Ok(Vec::new()) };
    let texto = String::from_utf8_lossy(&bytes);
    let todas: Vec<&str> = texto.lines().collect();
    Ok(todas[todas.len().saturating_sub(linhas)..].iter().map(|l| l.to_string()).collect())
}

#[tauri::command]
pub fn abrir_pasta_de_logs(app: AppHandle) -> Result<(), String> {
    let pasta = pasta(&app)?;
    std::fs::create_dir_all(&pasta).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer").arg(&pasta).spawn().map(drop).map_err(|e| e.to_string())
}
