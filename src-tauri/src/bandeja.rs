//! O app em segundo plano: ícone na área de notificação, fechar a janela sem sair e abrir com
//! o Windows. Escondida, a janela não recebe amostras, e os agentes leem no ritmo de pausa.

use std::sync::Arc;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::agentes::Agentes;

/// Argumento da entrada de abertura com o Windows: o app sobe só na área de notificação.
pub const ARGUMENTO: &str = "--segundo-plano";
const CHAVE_RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALOR_RUN: &str = "Toca do Pinguim";

pub fn mostrar(app: &AppHandle) {
    if let Some(janela) = app.get_webview_window("main") {
        let _ = janela.unminimize();
        let _ = janela.show();
        let _ = janela.set_focus();
    }
    app.state::<Arc<Agentes>>().pausar(false);
}

pub fn esconder(app: &AppHandle) {
    if let Some(janela) = app.get_webview_window("main") {
        let _ = janela.hide();
    }
    app.state::<Arc<Agentes>>().pausar(true);
}

pub fn montar(app: &tauri::App) -> tauri::Result<()> {
    let abrir = MenuItem::with_id(app, "abrir", "Abrir a Toca do Pinguim", true, None::<&str>)?;
    let sair = MenuItem::with_id(app, "sair", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&abrir, &sair])?;
    let mut icone = TrayIconBuilder::with_id("bandeja")
        .tooltip("Toca do Pinguim")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, evento| match evento.id().as_ref() {
            "abrir" => mostrar(app),
            "sair" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|icone, evento| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = evento {
                mostrar(icone.app_handle());
            }
        });
    if let Some(imagem) = app.default_window_icon() {
        icone = icone.icon(imagem.clone());
    }
    icone.build(app)?;
    Ok(())
}

#[derive(Serialize)]
pub struct SegundoPlano {
    pub fechar_para_bandeja: bool,
    pub iniciar_com_windows: bool,
}

fn largo(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// A linha gravada no Run: o executável atual, entre aspas, com o argumento de segundo plano.
fn comando() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(format!("\"{}\" {ARGUMENTO}", exe.display()))
}

/// Liga só se a entrada aponta para este executável: a de uma cópia portátil em outra pasta
/// não conta.
pub fn iniciar_com_windows() -> bool {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    let (chave, valor) = (largo(CHAVE_RUN), largo(VALOR_RUN));
    let mut buffer = vec![0u16; 2048];
    let mut tamanho = (buffer.len() * 2) as u32;
    // SAFETY: textos terminados em zero e buffer do tamanho informado.
    let ok = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, chave.as_ptr(), valor.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut tamanho)
    } == 0;
    let fim = buffer.iter().position(|&c| c == 0).unwrap_or(0);
    ok && comando().is_ok_and(|c| String::from_utf16_lossy(&buffer[..fim]).eq_ignore_ascii_case(&c))
}

pub fn definir_iniciar_com_windows(sim: bool) -> Result<(), String> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW};
    let (chave, valor) = (largo(CHAVE_RUN), largo(VALOR_RUN));
    // SAFETY: textos terminados em zero; o dado é o texto em UTF-16 com o zero do fim, no
    // tamanho em bytes que a API pede.
    let erro = unsafe {
        if sim {
            let dado = largo(&comando()?);
            RegSetKeyValueW(HKEY_CURRENT_USER, chave.as_ptr(), valor.as_ptr(), REG_SZ, dado.as_ptr().cast(), (dado.len() * 2) as u32)
        } else {
            match RegDeleteKeyValueW(HKEY_CURRENT_USER, chave.as_ptr(), valor.as_ptr()) {
                // 2: a entrada já não existia.
                2 => 0,
                e => e,
            }
        }
    };
    if erro == 0 { Ok(()) } else { Err(format!("não foi possível gravar a abertura com o Windows (erro {erro})")) }
}
