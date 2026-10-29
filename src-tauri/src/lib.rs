mod agentes;
mod atividade;
mod atualizacao;
mod config;
mod historico;
mod registro;
mod wsl;

use std::path::Path;
use std::sync::Arc;

use agentes::Agentes;
use log::{info, warn};
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

#[derive(Serialize)]
struct ConteudoWslconfig {
    caminho: String,
    existe: bool,
    /// (seção, chave, valor), na ordem do arquivo.
    valores: Vec<(String, String, String)>,
}

#[tauri::command]
fn ler_wslconfig() -> ConteudoWslconfig {
    let caminho = config::caminho();
    ConteudoWslconfig {
        existe: caminho.is_file(),
        caminho: caminho.display().to_string(),
        valores: WslConfig::ler().todas(),
    }
}

#[derive(serde::Deserialize)]
struct MudancaWslconfig {
    secao: String,
    chave: String,
    /// `None` tira a chave, e o WSL volta ao padrão dele.
    valor: Option<String>,
}

/// Seções que o `.wslconfig` documenta. Gravar fora delas não teria efeito e poderia
/// esconder um erro de digitação.
const SECOES_WSLCONFIG: [&str; 3] = ["wsl2", "experimental", "general"];

fn validar(m: &MudancaWslconfig) -> Result<(), String> {
    if !SECOES_WSLCONFIG.iter().any(|s| s.eq_ignore_ascii_case(&m.secao)) {
        return Err(format!("seção desconhecida: [{}]", m.secao));
    }
    if m.chave.is_empty() || !m.chave.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(format!("nome de chave inválido: {}", m.chave));
    }
    // Uma quebra de linha no valor viraria outra chave, ou outra seção, no arquivo.
    if m.valor.as_deref().is_some_and(|v| v.contains(['\n', '\r'])) {
        return Err(format!("{}: o valor não pode ter quebra de linha", m.chave));
    }
    Ok(())
}

/// Aplica as mudanças de uma vez e grava. Como no limite de memória, a mudança só vale
/// quando a VM reinicia; com `reiniciar`, o app desliga o WSL na hora.
#[tauri::command]
async fn gravar_wslconfig(
    app: tauri::AppHandle,
    estado: Estado<'_>,
    mudancas: Vec<MudancaWslconfig>,
    reiniciar: bool,
) -> Result<(), String> {
    mudancas.iter().try_for_each(validar)?;
    if mudancas.is_empty() {
        return Ok(());
    }
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        let mut cfg = WslConfig::ler();
        let mut resumo = Vec::new();
        for m in &mudancas {
            let antes = cfg.obter(&m.secao, &m.chave).unwrap_or_else(|| "padrão".into());
            match &m.valor {
                Some(v) => cfg.definir(&m.secao, &m.chave, v),
                None => cfg.remover(&m.secao, &m.chave),
            }
            resumo.push(format!("{}: {antes} para {}", m.chave, m.valor.as_deref().unwrap_or("padrão")));
        }
        cfg.gravar().map_err(|e| format!("não foi possível gravar o .wslconfig: {e}"))?;
        let resumo = resumo.join("; ");
        info!(".wslconfig alterado (reiniciar: {reiniciar}): {resumo}");
        agentes.registrar(&app, "config", None, format!(".wslconfig: {resumo}"));
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
        // Primeiro, para o log já pegar o que os outros plugins e o setup registram.
        .plugin(registro::plugin())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            ler_wslconfig,
            gravar_wslconfig,
            atualizacao::procurar_atualizacao,
            atualizacao::instalar_atualizacao,
            atualizacao::abrir_pagina_da_versao,
            registro::ler_log,
            registro::abrir_pasta_de_logs,
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
