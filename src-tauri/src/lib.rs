mod agentes;
mod atalho;
mod atividade;
mod atualizacao;
mod cache;
mod config;
mod configuracao;
mod docker;
mod historico;
mod recursos;
mod registro;
mod travados;
mod wsl;

use std::path::Path;
use std::sync::Arc;

use agentes::Agentes;
use log::{info, warn};
use atividade::Evento;
use config::WslConfig;
use recursos::Recursos;
use serde::Serialize;
use tauri::{Manager, RunEvent, State, WindowEvent};
use wsl::Distro;

pub(crate) type Estado<'a> = State<'a, Arc<Agentes>>;

const GIB: u64 = 1 << 30;

/// Os comandos que chamam `wsl.exe` levam de dezenas de milissegundos a segundos;
/// rodam fora da thread principal para a janela não congelar.
pub(crate) async fn bloqueante<T: Send + 'static>(
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
        info!("pedido: iniciar {nome}");
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
        info!("pedido: terminar {nome}");
        agentes.parar(&nome);
        wsl::terminar(&nome)
    })
    .await
}

#[tauri::command]
async fn reiniciar_distro(app: tauri::AppHandle, estado: Estado<'_>, nome: String) -> Result<(), String> {
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        info!("pedido: reiniciar {nome}");
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
        info!("pedido: desligar o WSL");
        agentes.parar_todos();
        wsl::desligar()?;
        agentes.registrar(&app, "wsl", None, "O app desligou o WSL".to_string());
        Ok(())
    })
    .await
}

#[tauri::command]
fn enviar_sinal(estado: Estado<'_>, distro: String, pid: i32, sinal: i32, nome: String) -> Result<(), String> {
    info!("pedido: sinal {sinal} para {nome} (PID {pid}) em {distro}");
    estado.enviar_sinal(&distro, pid, sinal, nome)
}

#[tauri::command]
fn definir_intervalo(estado: Estado<'_>, ms: u64) {
    estado.definir_intervalo(ms.max(250));
}

#[derive(Serialize)]
struct EstadoRecursos {
    memoria: recursos::MemoriaWindows,
    docker: recursos::Docker,
    saude: Option<wsl::saude::Situacao>,
    devolucao: Option<cache::Devolucao>,
    devolver_cache_automatico: bool,
}

/// O que as threads de fundo já sabem, para a tela que abre depois do último evento.
#[tauri::command]
fn estado_recursos(recursos: State<'_, Arc<Recursos>>) -> EstadoRecursos {
    EstadoRecursos {
        memoria: recursos.memoria.lock().unwrap().clone(),
        docker: recursos.docker.lock().unwrap().clone(),
        saude: wsl::saude::atual(),
        devolucao: recursos.devolucao.lock().unwrap().clone(),
        devolver_cache_automatico: recursos.preferencias.lock().unwrap().devolver_cache_automatico,
    }
}

#[tauri::command]
async fn devolver_cache(app: tauri::AppHandle, agentes: Estado<'_>, recursos: State<'_, Arc<Recursos>>) -> Result<(), String> {
    let (agentes, recursos) = (Arc::clone(&agentes), Arc::clone(&recursos));
    info!("pedido: devolver o cache da VM");
    bloqueante(move || cache::devolver(app, agentes, recursos, false)).await
}

#[tauri::command]
fn definir_devolver_cache_automatico(recursos: State<'_, Arc<Recursos>>, sim: bool) -> Result<(), String> {
    info!("devolver o cache sozinho: {}", if sim { "ligado" } else { "desligado" });
    recursos.definir_devolver_automatico(sim)
}

enum AcaoDocker {
    Parar,
    Ligar,
    Reiniciar,
}

async fn agir_em_conteineres(
    app: tauri::AppHandle,
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    ids: Vec<String>,
    rotulo: String,
    acao: AcaoDocker,
) -> Result<(), String> {
    recursos.conferir_ids(&ids)?;
    let (agentes, recursos) = (Arc::clone(&agentes), Arc::clone(&recursos));
    bloqueante(move || {
        let (verbo, feito, executar): (&str, &str, fn(&[String]) -> Result<(), String>) = match acao {
            AcaoDocker::Parar => ("parar", "parou", docker::parar),
            AcaoDocker::Ligar => ("ligar", "ligou", docker::iniciar),
            AcaoDocker::Reiniciar => ("reiniciar", "reiniciou", docker::reiniciar),
        };
        info!("pedido: {verbo} {rotulo} ({} container(s))", ids.len());
        let resultado = executar(&ids);
        recursos.pedir_atualizacao();
        resultado?;
        agentes.registrar(&app, "docker", Some("docker-desktop"), format!("O app {feito} {rotulo} ({} container(s))", ids.len()));
        Ok(())
    })
    .await
}

#[tauri::command]
async fn parar_conteineres(
    app: tauri::AppHandle,
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    ids: Vec<String>,
    rotulo: String,
) -> Result<(), String> {
    agir_em_conteineres(app, agentes, recursos, ids, rotulo, AcaoDocker::Parar).await
}

#[tauri::command]
async fn iniciar_conteineres(
    app: tauri::AppHandle,
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    ids: Vec<String>,
    rotulo: String,
) -> Result<(), String> {
    agir_em_conteineres(app, agentes, recursos, ids, rotulo, AcaoDocker::Ligar).await
}

#[tauri::command]
async fn reiniciar_conteineres(
    app: tauri::AppHandle,
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    ids: Vec<String>,
    rotulo: String,
) -> Result<(), String> {
    agir_em_conteineres(app, agentes, recursos, ids, rotulo, AcaoDocker::Reiniciar).await
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
        info!("limite de memória do WSL: {antes} para {gb}GB (reiniciar: {reiniciar})");
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

/// Identificador de quando o app se chamava Gerenciador de WSL; a pasta de dados vem dele.
const IDENTIFICADOR_ANTIGO: &str = "com.luan.wsltm";
const ARQUIVOS_DE_DADOS: [&str; 2] = ["memoria.csv", "atividade.jsonl"];

/// Copia o histórico da pasta do nome antigo, uma vez: arquivo que já existe na pasta nova
/// não é sobrescrito. A pasta antiga fica intacta, para quem voltar à versão anterior.
fn trazer_dados_antigos(dados: &Path) {
    let Some(antiga) = dados.parent().map(|p| p.join(IDENTIFICADOR_ANTIGO)) else { return };
    for nome in ARQUIVOS_DE_DADOS {
        let (de, para) = (antiga.join(nome), dados.join(nome));
        if de.is_file() && !para.exists() {
            if let Err(e) = std::fs::copy(&de, &para) {
                warn!("não copiei {} da pasta antiga: {e}", de.display());
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Uma segunda abertura (pela tecla de atalho, pelo menu) só traz esta janela para
        // frente: duas instâncias subiriam dois agentes em cada distro.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(janela) = app.get_webview_window("main") {
                let _ = janela.unminimize();
                let _ = janela.show();
                let _ = janela.set_focus();
            }
        }))
        // Logo depois, para o log já pegar o que os outros plugins e o setup registram.
        .plugin(registro::plugin())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            info!("Toca do Pinguim {} iniciado", app.package_info().version);
            // Versão do WSL e do kernel, para o log servir de diagnóstico sem perguntar.
            std::thread::spawn(|| match wsl::versao() {
                Ok(v) => info!("wsl --version: {}", v.split_whitespace().collect::<Vec<_>>().join(" ")),
                Err(e) => warn!("wsl --version falhou: {e}"),
            });
            let dados = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dados)?;
            trazer_dados_antigos(&dados);
            let agentes = Agentes::novo(&dados);
            app.manage(Arc::clone(&agentes));
            let recursos = Arc::new(Recursos::novo(&dados));
            app.manage(Arc::clone(&recursos));
            recursos::vigiar(app.handle().clone(), Arc::clone(&agentes), recursos);
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
            estado_recursos,
            parar_conteineres,
            iniciar_conteineres,
            reiniciar_conteineres,
            devolver_cache,
            definir_devolver_cache_automatico,
            definir_limite_memoria,
            configuracao::ler_wslconfig,
            configuracao::gravar_wslconfig,
            configuracao::ler_wslconf,
            configuracao::gravar_wslconf,
            atualizacao::procurar_atualizacao,
            atualizacao::instalar_atualizacao,
            atualizacao::abrir_pagina_da_versao,
            registro::ler_log,
            registro::abrir_pasta_de_logs,
            atalho::ler_atalho_teclado,
            atalho::definir_atalho_teclado,
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

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn traz_da_pasta_antiga_sem_sobrescrever() {
        let raiz = std::env::temp_dir().join(format!("toca-migracao-{}", std::process::id()));
        let (antiga, nova) = (raiz.join(IDENTIFICADOR_ANTIGO), raiz.join("com.luan.tocadopinguim"));
        std::fs::create_dir_all(&antiga).unwrap();
        std::fs::create_dir_all(&nova).unwrap();
        std::fs::write(antiga.join("memoria.csv"), "antigo").unwrap();
        std::fs::write(antiga.join("atividade.jsonl"), "antigo").unwrap();
        std::fs::write(nova.join("atividade.jsonl"), "novo").unwrap();

        trazer_dados_antigos(&nova);

        assert_eq!(std::fs::read_to_string(nova.join("memoria.csv")).unwrap(), "antigo");
        assert_eq!(std::fs::read_to_string(nova.join("atividade.jsonl")).unwrap(), "novo");
        assert!(antiga.join("memoria.csv").is_file());
        std::fs::remove_dir_all(&raiz).unwrap();
    }
}
