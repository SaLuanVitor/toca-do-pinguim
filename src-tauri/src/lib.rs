mod agentes;
mod wsl;

use std::sync::Arc;

use agentes::Agentes;
use protocolo::Comando;
use tauri::{Manager, RunEvent, State, WindowEvent};
use wsl::Distro;

type Estado<'a> = State<'a, Arc<Agentes>>;

/// Os comandos que chamam `wsl.exe` levam de dezenas de milissegundos a segundos;
/// rodam fora da thread principal para a janela não congelar.
async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn listar_distros() -> Result<Vec<Distro>, String> {
    bloqueante(wsl::listar).await
}

#[tauri::command]
async fn iniciar_distro(app: tauri::AppHandle, estado: Estado<'_>, nome: String) -> Result<(), String> {
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        wsl::iniciar(&nome)?;
        // O agente é o que mantém a distro de pé depois que o `true` sai.
        agentes.garantir(&app, &nome);
        Ok(())
    })
    .await
}

#[tauri::command]
async fn terminar_distro(estado: Estado<'_>, nome: String) -> Result<(), String> {
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        agentes.parar(&nome);
        wsl::terminar(&nome)
    })
    .await
}

#[tauri::command]
async fn reiniciar_distro(app: tauri::AppHandle, estado: Estado<'_>, nome: String) -> Result<(), String> {
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        agentes.parar(&nome);
        wsl::terminar(&nome)?;
        wsl::iniciar(&nome)?;
        agentes.garantir(&app, &nome);
        Ok(())
    })
    .await
}

#[tauri::command]
async fn desligar_wsl(estado: Estado<'_>) -> Result<(), String> {
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        agentes.parar_todos();
        wsl::desligar()
    })
    .await
}

#[tauri::command]
fn enviar_sinal(estado: Estado<'_>, distro: String, pid: i32, sinal: i32) -> Result<(), String> {
    estado.enviar(&distro, &Comando::Sinal { pid, sinal })
}

#[tauri::command]
fn definir_intervalo(estado: Estado<'_>, ms: u64) {
    estado.definir_intervalo(ms.max(250));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let agentes = Agentes::novo();
    tauri::Builder::default()
        .manage(Arc::clone(&agentes))
        .setup(move |app| {
            agentes::supervisionar(app.handle().clone(), agentes);
            Ok(())
        })
        .on_window_event(|janela, evento| {
            // Minimizar chega como redimensionamento; restaurar também.
            if let WindowEvent::Resized(_) = evento {
                let minimizada = janela.is_minimized().unwrap_or(false);
                janela.state::<Arc<Agentes>>().pausar(minimizada);
            }
        })
        .invoke_handler(tauri::generate_handler![
            listar_distros,
            iniciar_distro,
            terminar_distro,
            reiniciar_distro,
            desligar_wsl,
            enviar_sinal,
            definir_intervalo,
        ])
        .build(tauri::generate_context!())
        .expect("falha ao montar o app")
        .run(|app, evento| {
            if let RunEvent::Exit = evento {
                app.state::<Arc<Agentes>>().parar_todos();
            }
        });
}
