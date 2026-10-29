//! Procura versão nova no `latest.json` da release mais recente do GitHub. Nada é baixado
//! sem a pessoa pedir: a busca só informa, e a instalação vem de um clique.

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::{Updater, UpdaterExt};

#[derive(Serialize)]
pub struct Atualizacao {
    versao: String,
    notas: Option<String>,
    /// Falso na versão portátil, que não se substitui sozinha: nela o app só aponta a
    /// página de releases.
    instalavel: bool,
}

fn atualizador(app: &AppHandle) -> Result<Updater, String> {
    app.updater().map_err(|e| e.to_string())
}

/// O instalador grava `uninstall.exe` ao lado do executável; o zip portátil não tem.
fn instalado_pelo_instalador() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("uninstall.exe").is_file()))
        .unwrap_or(false)
}

#[tauri::command]
pub async fn procurar_atualizacao(app: AppHandle) -> Result<Option<Atualizacao>, String> {
    let nova = atualizador(&app)?.check().await.map_err(|e| e.to_string())?;
    Ok(nova.map(|u| Atualizacao { versao: u.version, notas: u.body, instalavel: instalado_pelo_instalador() }))
}

/// Baixa, confere a assinatura contra a chave pública do `tauri.conf.json` e roda o
/// instalador, que fecha o app e o abre de novo na versão nova.
#[tauri::command]
pub async fn instalar_atualizacao(app: AppHandle) -> Result<(), String> {
    if !instalado_pelo_instalador() {
        return Err("A versão portátil não se atualiza sozinha; baixe a nova na página de releases.".into());
    }
    let Some(nova) = atualizador(&app)?.check().await.map_err(|e| e.to_string())? else {
        return Err("Não há versão nova.".into());
    };
    nova.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    app.restart()
}

const RELEASES: &str = "https://github.com/SaLuanVitor/toca-do-pinguim/releases";

/// Abre a página da versão no navegador padrão. A versão vem do `latest.json` e vira
/// parte de uma URL passada ao Explorer, então só passa se for número de versão.
#[tauri::command]
pub fn abrir_pagina_da_versao(versao: String) -> Result<(), String> {
    if versao.is_empty() || !versao.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')) {
        return Err(format!("versão inválida: {versao}"));
    }
    std::process::Command::new("explorer")
        .arg(format!("{RELEASES}/tag/v{versao}"))
        .spawn()
        .map(drop)
        .map_err(|e| e.to_string())
}
