mod agentes;
mod atividade;
mod config;
mod historico;
mod wsl;

use std::sync::Arc;

use agentes::Agentes;
use atividade::Evento;
use config::WslConfig;
use serde::Serialize;
use tauri::{Manager, RunEvent, State, WindowEvent};
use wsl::Distro;

type Estado<'a> = State<'a, Arc<Agentes>>;

const GIB: u64 = 1 << 30;

/// Os comandos que chamam `wsl.exe` levam de dezenas de milissegundos a segundos;
/// rodam fora da thread principal para a janela não congelar.
async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

fn ram_host() -> u64 {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: estrutura zerada com `dwLength` preenchido, como a API exige.
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut m) } != 0 { m.ullTotalPhys } else { 0 }
}

#[derive(Serialize)]
struct ResumoMemoria {
    limite_bytes: u64,
    /// Falso quando o `.wslconfig` não define `memory` e vale o padrão do WSL.
    limite_configurado: bool,
    ram_host_bytes: u64,
    minutos_de_dados: usize,
    pico_bytes: u64,
    sugestao_bytes: Option<u64>,
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
async fn desligar_wsl(app: tauri::AppHandle, estado: Estado<'_>) -> Result<(), String> {
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        agentes.parar_todos();
        wsl::desligar()?;
        agentes.registrar(&app, "wsl", None, "O app desligou o WSL".to_string());
        Ok(())
    })
    .await
}

#[tauri::command]
fn enviar_sinal(estado: Estado<'_>, distro: String, pid: i32, sinal: i32, nome: String) -> Result<(), String> {
    estado.enviar_sinal(&distro, pid, sinal, nome)
}

#[tauri::command]
fn definir_intervalo(estado: Estado<'_>, ms: u64) {
    estado.definir_intervalo(ms.max(250));
}

#[tauri::command]
fn listar_atividade(estado: Estado<'_>) -> Vec<Evento> {
    estado.atividade.lock().unwrap().listar()
}

#[tauri::command]
fn resumo_memoria(estado: Estado<'_>) -> ResumoMemoria {
    let ram = ram_host();
    let configurado = WslConfig::ler()
        .obter("wsl2", "memory")
        .and_then(|v| config::tamanho_em_bytes(&v));
    let r = estado.historico.lock().unwrap().resumo(ram);
    ResumoMemoria {
        // Sem `memory` no arquivo, o WSL usa metade da RAM do Windows.
        limite_bytes: configurado.unwrap_or(ram / 2),
        limite_configurado: configurado.is_some(),
        ram_host_bytes: ram,
        minutos_de_dados: r.minutos_de_dados,
        pico_bytes: r.pico_bytes,
        sugestao_bytes: r.sugestao_bytes,
    }
}

/// Grava `memory` no `.wslconfig`. A mudança só vale quando a VM reinicia; com
/// `reiniciar`, o app desliga o WSL na hora, senão ela vale no próximo início.
#[tauri::command]
async fn definir_limite_memoria(
    app: tauri::AppHandle,
    estado: Estado<'_>,
    gb: u64,
    reiniciar: bool,
) -> Result<(), String> {
    let ram_gb = ram_host() / GIB;
    if gb < 1 || gb > ram_gb {
        return Err(format!("o limite precisa ficar entre 1 e {ram_gb} GB"));
    }
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        let mut cfg = WslConfig::ler();
        let antes = cfg.obter("wsl2", "memory").unwrap_or_else(|| "padrão (metade da RAM)".to_string());
        cfg.definir("wsl2", "memory", &format!("{gb}GB"));
        cfg.gravar().map_err(|e| format!("não foi possível gravar o .wslconfig: {e}"))?;
        agentes.registrar(&app, "config", None, format!("Limite de memória do WSL: {antes} para {gb}GB"));
        if reiniciar {
            agentes.parar_todos();
            wsl::desligar()?;
            agentes.registrar(&app, "wsl", None, "O app desligou o WSL para aplicar a configuração".to_string());
        }
        Ok(())
    })
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dados = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dados)?;
            let agentes = Agentes::novo(&dados);
            app.manage(Arc::clone(&agentes));
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
            listar_atividade,
            resumo_memoria,
            definir_limite_memoria,
        ])
        .build(tauri::generate_context!())
        .expect("falha ao montar o app")
        .run(|app, evento| {
            if let RunEvent::Exit = evento {
                if let Some(agentes) = app.try_state::<Arc<Agentes>>() {
                    agentes.parar_todos();
                }
            }
        });
}
