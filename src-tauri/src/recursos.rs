//! Memória do Windows e o que a VM do WSL ocupa nela.
//!
//! Medido numa máquina de 32 GB: com o Windows a 2,7 GB livres e a VM no teto do
//! `.wslconfig`, o WSL parou de responder (`Wsl/Service/0x8007274c`) e só voltou com
//! `wsl --shutdown`. O app acompanha a memória do Windows para avisar antes disso, e
//! mostra quem dentro da VM ocupa mais, por projeto do Docker.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use log::{info, warn};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use crate::agentes::Agentes;
use crate::armazenamento::{self, Analise, Volume};
use crate::cache::{self, Devolucao, LimpezaProgramada, Preferencias};
use crate::docker::{self, Conteiner};

const PAUSA: Duration = Duration::from_millis(500);
/// A limpeza programada roda uma vez por semana; a conferência de se já está na hora, a cada
/// meia hora.
const SEMANA_MS: u64 = 7 * 24 * 60 * 60 * 1000;
const A_CADA_LIMPEZA: Duration = Duration::from_secs(30 * 60);
const A_CADA_MEMORIA: Duration = Duration::from_secs(5);
const A_CADA_DOCKER: Duration = Duration::from_secs(15);
const DISTRO_DOCKER: &str = "docker-desktop";

/// Abaixo disto de memória disponível o Windows entra em pressão, e sai acima do segundo
/// limiar; a folga entre os dois impede o aviso de piscar perto da linha.
const ENTRA_EM_PRESSAO: f64 = 0.10;
const SAI_DA_PRESSAO: f64 = 0.15;

#[derive(Serialize, Clone, Debug, Default)]
pub struct MemoriaWindows {
    pub total_bytes: u64,
    pub disponivel_bytes: u64,
    /// Memória comprometida e o limite dela (RAM mais arquivo de paginação).
    pub commit_bytes: u64,
    pub commit_limite_bytes: u64,
    /// Memória física (working set) do `vmmemWSL`: quanto da RAM do Windows a VM inteira
    /// ocupa, cache incluso. É o número que o Gerenciador de Tarefas mostra para ela.
    /// `None` com o WSL desligado.
    pub wsl_bytes: Option<u64>,
    pub pressao: bool,
}

#[derive(Serialize, Clone, Default)]
pub struct Docker {
    pub ligado: bool,
    pub conteineres: Vec<Conteiner>,
    pub erro: Option<String>,
}

pub struct Recursos {
    pub memoria: Mutex<MemoriaWindows>,
    pub docker: Mutex<Docker>,
    /// Pedido de nova leitura do Docker logo depois de parar ou ligar containers.
    atualizar_docker: AtomicBool,
    pub devolvendo: AtomicBool,
    pub ultima_devolucao: Mutex<Option<Instant>>,
    pub devolucao: Mutex<Option<Devolucao>>,
    pub preferencias: Mutex<Preferencias>,
    caminho_preferencias: PathBuf,
    /// Última análise de pastas de cada distro: só se apaga o que ela marcou como apagável.
    pub analises: Mutex<std::collections::HashMap<String, Analise>>,
    /// Volumes sem uso da última leitura do Docker, os únicos que a limpeza aceita.
    pub volumes_sem_uso: Mutex<Vec<Volume>>,
}

impl Recursos {
    pub fn novo(dados: &Path) -> Self {
        let caminho_preferencias = Preferencias::caminho(dados);
        Recursos {
            memoria: Mutex::default(),
            docker: Mutex::default(),
            atualizar_docker: AtomicBool::new(false),
            devolvendo: AtomicBool::new(false),
            ultima_devolucao: Mutex::new(None),
            devolucao: Mutex::new(None),
            preferencias: Mutex::new(Preferencias::ler(&caminho_preferencias)),
            caminho_preferencias,
            analises: Mutex::default(),
            volumes_sem_uso: Mutex::default(),
        }
    }

    /// Grava a limpeza escolhida, mantendo a data da última que rodou.
    pub fn definir_limpeza(&self, nova: LimpezaProgramada) -> Result<(), String> {
        let mut prefs = self.preferencias.lock().unwrap();
        let limpeza = LimpezaProgramada { ultima_ms: prefs.limpeza.ultima_ms, ..nova };
        let novas = Preferencias { limpeza, ..prefs.clone() };
        novas.gravar(&self.caminho_preferencias)?;
        *prefs = novas;
        Ok(())
    }

    fn marcar_limpeza(&self, quando_ms: u64) {
        let mut prefs = self.preferencias.lock().unwrap();
        prefs.limpeza.ultima_ms = Some(quando_ms);
        if let Err(e) = prefs.gravar(&self.caminho_preferencias) {
            warn!("data da limpeza não gravada: {e}");
        }
    }

    pub fn definir_devolucao(&self, sim: bool, gatilho: cache::Gatilho) -> Result<(), String> {
        let mut prefs = self.preferencias.lock().unwrap();
        let novas = Preferencias { devolver_cache_automatico: sim, gatilho, ..prefs.clone() };
        novas.gravar(&self.caminho_preferencias)?;
        *prefs = novas;
        Ok(())
    }

    pub fn pedir_atualizacao(&self) {
        self.atualizar_docker.store(true, Ordering::Relaxed);
    }

    /// Só aceita ids da última leitura: o front não manda parar o que o app não mostrou.
    pub fn conferir_ids(&self, ids: &[String]) -> Result<(), String> {
        let docker = self.docker.lock().unwrap();
        match ids.iter().find(|id| !docker.conteineres.iter().any(|c| &c.id == *id)) {
            Some(id) => Err(format!("container {id} não está na lista atual")),
            None => Ok(()),
        }
    }
}

fn medir() -> MemoriaWindows {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: estrutura zerada com `dwLength` preenchido, como a API exige.
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 {
        return MemoriaWindows::default();
    }
    MemoriaWindows {
        total_bytes: m.ullTotalPhys,
        disponivel_bytes: m.ullAvailPhys,
        commit_bytes: m.ullTotalPageFile - m.ullAvailPageFile,
        commit_limite_bytes: m.ullTotalPageFile,
        wsl_bytes: memoria_da_vm(),
        pressao: false,
    }
}

/// O processo `vmmemWSL` representa a VM no Windows. Ele não aceita ser aberto sem
/// administrador, nem com o acesso de consulta limitada (medido: erro 5), então a memória
/// vem da lista de processos do sistema, que traz o número sem abrir o processo.
///
/// O número é o working set, a RAM que o processo ocupa agora. A memória privada (commit)
/// passa dele quando o Windows tira páginas da VM da RAM: medido, 11,57 GB de commit contra
/// 8,48 GB na RAM, com o Gerenciador de Tarefas mostrando os 8,5.
pub fn memoria_da_vm() -> Option<u64> {
    memoria_fisica_de("vmmemWSL")
}

/// Pelo nome da imagem, com a extensão: "Docker Desktop.exe".
pub fn processo_rodando(nome: &str) -> bool {
    memoria_fisica_de(nome).is_some()
}

fn memoria_fisica_de(processo: &str) -> Option<u64> {
    use windows_sys::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessInformation};
    use windows_sys::Win32::System::WindowsProgramming::SYSTEM_PROCESS_INFORMATION;

    const TAMANHO_INSUFICIENTE: i32 = 0xC000_0004_u32 as i32;
    // Em u64 para o alinhamento que a estrutura pede; a lista cresce entre duas chamadas,
    // então o tamanho pedido ganha folga.
    let mut buffer: Vec<u64> = vec![0; 64 * 1024];
    loop {
        let mut preciso = 0u32;
        let bytes = (buffer.len() * 8) as u32;
        // SAFETY: o buffer tem o tamanho informado e vive até o fim da leitura.
        let status = unsafe { NtQuerySystemInformation(SystemProcessInformation, buffer.as_mut_ptr().cast(), bytes, &mut preciso) };
        if status == TAMANHO_INSUFICIENTE && preciso > bytes {
            buffer = vec![0; (preciso as usize + 64 * 1024) / 8 + 1];
            continue;
        }
        if status < 0 {
            return None;
        }
        break;
    }
    let base = buffer.as_ptr().cast::<u8>();
    let mut desvio = 0usize;
    loop {
        // SAFETY: cada entrada começa no desvio que a anterior informou, dentro do buffer
        // que o sistema preencheu; o nome aponta para dentro do mesmo buffer.
        let entrada = unsafe { &*base.add(desvio).cast::<SYSTEM_PROCESS_INFORMATION>() };
        let nome = &entrada.ImageName;
        if !nome.Buffer.is_null() {
            let texto = unsafe { std::slice::from_raw_parts(nome.Buffer, nome.Length as usize / 2) };
            if String::from_utf16_lossy(texto).eq_ignore_ascii_case(processo) {
                return Some(entrada.WorkingSetSize as u64);
            }
        }
        if entrada.NextEntryOffset == 0 {
            return None;
        }
        desvio += entrada.NextEntryOffset as usize;
    }
}

/// Devolve o novo estado quando ele vira.
fn virada(em_pressao: bool, total: u64, disponivel: u64) -> Option<bool> {
    if total == 0 {
        return None;
    }
    let fracao = disponivel as f64 / total as f64;
    match em_pressao {
        false if fracao < ENTRA_EM_PRESSAO => Some(true),
        true if fracao >= SAI_DA_PRESSAO => Some(false),
        _ => None,
    }
}

/// O projeto (ou container avulso) que mais ocupa memória, para o aviso apontar por onde
/// começar.
pub fn maior_consumidor(conteineres: &[Conteiner]) -> Option<(String, u64)> {
    let mut soma: Vec<(String, u64)> = Vec::new();
    for c in conteineres {
        let chave = c.projeto.clone().unwrap_or_else(|| c.nome.clone());
        match soma.iter_mut().find(|(k, _)| *k == chave) {
            Some((_, total)) => *total += c.memoria_bytes,
            None => soma.push((chave, c.memoria_bytes)),
        }
    }
    soma.into_iter().filter(|(_, b)| *b > 0).max_by_key(|(_, b)| *b)
}

/// Quem ocupa a RAM do Windows. O WSL só é apontado quando passa do resto do Windows somado:
/// medido numa máquina com 31,7 GB, o WSL tinha 8,9 GB, Chrome e outros programas o resto, e
/// mandar baixar o limite do WSL olhava para o lugar errado. Dentro da VM, quando o cache passa
/// da memória somada dos containers, apontar o maior container também engana: a VM tinha
/// 10,1 GB de cache e o maior container, 219 MB.
pub fn explicar_wsl(usada: u64, wsl: u64, cache: Option<u64>, conteineres: &[Conteiner]) -> String {
    let outros = usada.saturating_sub(wsl);
    if wsl < outros {
        return format!(
            "A maior parte está com os programas do Windows ({} GB); o WSL ocupa {} GB. Baixar o limite do WSL libera pouco aqui.",
            gb(outros),
            gb(wsl)
        );
    }
    let docker: u64 = conteineres.iter().map(|c| c.memoria_bytes).sum();
    match (cache, maior_consumidor(conteineres)) {
        (Some(cache), _) if cache >= docker => format!(
            "O WSL ocupa {} GB, e a maior parte é cache de arquivos ({} GB), que o Linux devolve quando precisa mas o Windows conta como ocupado. Baixar o limite de memória do WSL, em Desempenho > Memória, segura esse tamanho.",
            gb(wsl),
            gb(cache)
        ),
        (_, Some((quem, bytes))) => format!("O WSL ocupa {} GB; quem mais usa dentro dele é {quem}, com {} GB.", gb(wsl), gb(bytes)),
        _ => format!("O WSL ocupa {} GB.", gb(wsl)),
    }
}

pub fn gb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / (1u64 << 30) as f64).replace('.', ",")
}

pub fn notificar(app: &AppHandle, titulo: &str, texto: &str) {
    if let Err(e) = app.notification().builder().title(titulo).body(texto).show() {
        warn!("notificação não exibida: {e}");
    }
}

pub fn vigiar(app: AppHandle, agentes: Arc<Agentes>, recursos: Arc<Recursos>) {
    thread::spawn(move || {
        let mut vigia = docker::Vigia::default();
        let mut problemas_antes: Vec<(String, Problema)> = Vec::new();
        let mut ultima_memoria: Option<Instant> = None;
        let mut ultima_docker: Option<Instant> = None;
        let mut conferiu_limpeza: Option<Instant> = None;
        loop {
            if conferiu_limpeza.is_none_or(|t| t.elapsed() >= A_CADA_LIMPEZA) {
                conferiu_limpeza = Some(Instant::now());
                let limpeza = recursos.preferencias.lock().unwrap().limpeza.clone();
                if hora_da_limpeza(&limpeza, agora_ms()) {
                    let (app, agentes, recursos) = (app.clone(), Arc::clone(&agentes), Arc::clone(&recursos));
                    thread::spawn(move || limpar_programado(&app, &agentes, &recursos, &limpeza));
                }
            }
            if ultima_memoria.is_none_or(|t| t.elapsed() >= A_CADA_MEMORIA) {
                ultima_memoria = Some(Instant::now());
                let mut m = medir();
                let antes = recursos.memoria.lock().unwrap().pressao;
                m.pressao = antes;
                if let Some(agora) = virada(antes, m.total_bytes, m.disponivel_bytes) {
                    m.pressao = agora;
                    avisar_pressao(&app, &agentes, &recursos, &m);
                }
                *recursos.memoria.lock().unwrap() = m.clone();
                let _ = app.emit("memoria_windows", &m);
                let (automatico, gatilho) = {
                    let p = recursos.preferencias.lock().unwrap();
                    (p.devolver_cache_automatico, p.gatilho.clone())
                };
                let desde = recursos.ultima_devolucao.lock().unwrap().map(|t| t.elapsed());
                let livre = if m.total_bytes > 0 { m.disponivel_bytes as f64 / m.total_bytes as f64 } else { 1.0 };
                if automatico
                    && !recursos.devolvendo.load(Ordering::Relaxed)
                    && cache::pode_devolver_sozinho(m.wsl_bytes.is_some(), livre, agentes.cache_vm(), &gatilho, desde)
                {
                    let (app, agentes, recursos) = (app.clone(), Arc::clone(&agentes), Arc::clone(&recursos));
                    thread::spawn(move || {
                        let _ = cache::devolver(app, agentes, recursos, true);
                    });
                }
            }

            let ligado = agentes.rodando(DISTRO_DOCKER);
            let pedido = recursos.atualizar_docker.swap(false, Ordering::Relaxed);
            // Minimizado, ninguém olha a tabela; o Docker só é lido se o aviso de pressão
            // precisar apontar quem ocupa mais.
            let quer = !agentes.pausado() || recursos.memoria.lock().unwrap().pressao;
            let vencido = ultima_docker.is_none_or(|t| t.elapsed() >= A_CADA_DOCKER);
            if ligado && (pedido || (quer && vencido)) {
                ultima_docker = Some(Instant::now());
                let estado = match docker::listar(&mut vigia) {
                    Ok(conteineres) => Docker { ligado, conteineres, erro: None },
                    Err(erro) => {
                        warn!("leitura do Docker falhou: {erro}");
                        Docker { ligado, conteineres: Vec::new(), erro: Some(erro) }
                    }
                };
                registrar_problemas(&app, &agentes, &estado.conteineres, &mut problemas_antes);
                *recursos.docker.lock().unwrap() = estado.clone();
                let _ = app.emit("docker", &estado);
            } else if !ligado && recursos.docker.lock().unwrap().ligado {
                ultima_docker = None;
                problemas_antes.clear();
                *recursos.docker.lock().unwrap() = Docker::default();
                let _ = app.emit("docker", Docker::default());
            }
            thread::sleep(PAUSA);
        }
    });
}

fn agora_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn hora_da_limpeza(l: &LimpezaProgramada, agora: u64) -> bool {
    l.ligada() && l.ultima_ms.is_none_or(|u| agora.saturating_sub(u) >= SEMANA_MS)
}

/// Roda o que estiver ligado e possível agora: a parte do Docker só com o Docker de pé, a do
/// `/tmp` em cada distro já ligada. Só marca a data se alguma parte rodou.
fn limpar_programado(app: &AppHandle, agentes: &Agentes, recursos: &Recursos, l: &LimpezaProgramada) {
    let docker = agentes.rodando(DISTRO_DOCKER);
    let mut feito: Vec<String> = Vec::new();
    let mut liberado = 0u64;
    let mut rodou = false;
    let montagens = if docker { armazenamento::resumo_docker().map(|(_, m)| m).unwrap_or_default() } else { Vec::new() };
    if docker && l.cache_de_build {
        rodou = true;
        match armazenamento::limpar_docker(armazenamento::LimpezaDocker::CacheDeBuild, &[], &[]) {
            Ok(b) => {
                liberado += b;
                feito.push(format!("cache de build do Docker ({} GB)", gb(b)));
            }
            Err(e) => warn!("limpeza programada do cache de build falhou: {e}"),
        }
    }
    if docker && l.sobras_de_containers {
        rodou = true;
        match armazenamento::limpar_docker(armazenamento::LimpezaDocker::Sobras, &[], &[]) {
            Ok(b) => {
                liberado += b;
                feito.push(format!("containers parados de builds ({} GB)", gb(b)));
            }
            Err(e) => warn!("limpeza programada dos containers falhou: {e}"),
        }
    }
    if let Some(dias) = l.tmp_dias {
        for distro in agentes.ligadas().into_iter().filter(|d| d != DISTRO_DOCKER) {
            rodou = true;
            match armazenamento::limpar_tmp_antigo(&distro, dias, &armazenamento::em_uso_na_distro(&distro, &montagens)) {
                Ok(b) => {
                    liberado += b;
                    feito.push(format!("/tmp de {distro} com mais de {dias} dias ({} GB)", gb(b)));
                }
                Err(e) => warn!("limpeza programada do /tmp de {distro} falhou: {e}"),
            }
        }
    }
    if rodou {
        recursos.marcar_limpeza(agora_ms());
        let texto = format!("Limpeza programada: {} GB liberados ({})", gb(liberado), feito.join("; "));
        info!("{texto}");
        agentes.registrar(app, "armazenamento", None, texto);
    }
}

fn avisar_pressao(app: &AppHandle, agentes: &Agentes, recursos: &Recursos, m: &MemoriaWindows) {
    if !m.pressao {
        info!("memória do Windows voltou: {} GB disponíveis", gb(m.disponivel_bytes));
        agentes.registrar(app, "memoria", None, format!("A memória do Windows voltou a ter folga: {} GB disponíveis", gb(m.disponivel_bytes)));
        return;
    }
    let mut texto = format!("{} GB livres de {} GB.", gb(m.disponivel_bytes), gb(m.total_bytes));
    if let Some(wsl) = m.wsl_bytes {
        texto.push(' ');
        let usada = m.total_bytes.saturating_sub(m.disponivel_bytes);
        texto.push_str(&explicar_wsl(usada, wsl, agentes.cache_vm(), &recursos.docker.lock().unwrap().conteineres));
    }
    warn!("memória do Windows acabando: {texto}");
    agentes.registrar(app, "memoria", None, format!("A memória do Windows está acabando: {texto}"));
    notificar(app, "A memória do Windows está acabando", &texto);
}

#[derive(Clone, Copy, PartialEq)]
enum Problema {
    Ciclo,
    SemResposta,
    PertoDoLimite,
}

fn problemas(c: &Conteiner) -> Vec<Problema> {
    [(c.em_ciclo, Problema::Ciclo), (c.sem_resposta(), Problema::SemResposta), (c.perto_do_limite(), Problema::PertoDoLimite)]
        .into_iter()
        .filter_map(|(sim, p)| sim.then_some(p))
        .collect()
}

/// Cada problema vai uma vez para a Atividade, quando aparece. Notificação só para os sinais
/// firmes: reinício em ciclo e healthcheck falhando.
fn registrar_problemas(app: &AppHandle, agentes: &Agentes, conteineres: &[Conteiner], antes: &mut Vec<(String, Problema)>) {
    let mut agora = Vec::new();
    for c in conteineres {
        for p in problemas(c) {
            agora.push((c.id.clone(), p));
            if antes.contains(&(c.id.clone(), p)) {
                continue;
            }
            let (texto, notificar_tambem) = match p {
                Problema::Ciclo => (format!("O container {} está reiniciando sem parar ({} reinícios)", c.nome, c.reinicios), true),
                Problema::SemResposta => (format!("O container {} não responde ao próprio healthcheck", c.nome), true),
                Problema::PertoDoLimite => (
                    format!("O container {} usa {} GB de um limite de {} GB", c.nome, gb(c.memoria_bytes), gb(c.limite_bytes)),
                    false,
                ),
            };
            agentes.registrar(app, "docker", Some(DISTRO_DOCKER), texto.clone());
            if notificar_tambem {
                notificar(app, "Container com problema", &texto);
            }
        }
    }
    *antes = agora;
}

#[cfg(test)]
mod testes {
    use super::*;

    const GIB: u64 = 1 << 30;

    #[test]
    fn pressao_entra_abaixo_de_10_e_sai_acima_de_15_por_cento() {
        let total = 32 * GIB;
        assert_eq!(virada(false, total, 4 * GIB), None);
        assert_eq!(virada(false, total, 3 * GIB), Some(true));
        // Entre os dois limiares, nada muda em nenhum dos sentidos.
        assert_eq!(virada(true, total, 4 * GIB), None);
        assert_eq!(virada(false, total, 4 * GIB), None);
        assert_eq!(virada(true, total, 5 * GIB), Some(false));
        assert_eq!(virada(false, 0, 0), None);
    }

    #[test]
    fn maior_consumidor_soma_por_projeto() {
        let c = |nome: &str, projeto: Option<&str>, mb: u64| Conteiner {
            id: String::new(),
            nome: nome.to_string(),
            projeto: projeto.map(str::to_string),
            estado: "running".to_string(),
            memoria_bytes: mb << 20,
            limite_bytes: 0,
            reinicios: 0,
            em_ciclo: false,
            saude: None,
        };
        let lista = [c("a", Some("loja"), 1700), c("b", Some("loja"), 700), c("c", Some("chat-bot"), 2000), c("runner", None, 30)];
        assert_eq!(maior_consumidor(&lista), Some(("loja".to_string(), 2400 << 20)));
        assert_eq!(maior_consumidor(&[]), None);
    }

    #[test]
    fn acha_processo_na_lista_do_sistema() {
        assert!(memoria_fisica_de("System").is_some());
        assert_eq!(memoria_fisica_de("processo-que-nao-existe.exe"), None);
    }

    #[test]
    fn aviso_aponta_o_cache_quando_ele_pesa_mais_que_os_containers() {
        let c = |nome: &str, mb: u64| Conteiner {
            id: String::new(),
            nome: nome.to_string(),
            projeto: None,
            estado: "running".to_string(),
            memoria_bytes: mb << 20,
            limite_bytes: 0,
            reinicios: 0,
            em_ciclo: false,
            saude: None,
        };
        let runner = [c("runner-build", 219)];
        assert!(explicar_wsl(12 * GIB, 8 * GIB, Some(10 * GIB), &runner).contains("cache de arquivos (10,0 GB)"));
        let loja = [c("loja-db", 4 * 1024)];
        assert!(explicar_wsl(12 * GIB, 8 * GIB, Some(2 * GIB), &loja).contains("quem mais usa dentro dele é loja-db"));
        assert_eq!(explicar_wsl(8 * GIB, 8 * GIB, None, &[]), "O WSL ocupa 8,0 GB.");
    }

    #[test]
    fn aviso_nao_culpa_o_wsl_quando_o_windows_ocupa_mais() {
        let texto = explicar_wsl(31 * GIB, 9 * GIB, Some(7 * GIB), &[]);
        assert!(texto.starts_with("A maior parte está com os programas do Windows (22,0 GB)"), "{texto}");
        assert!(!texto.contains("cache de arquivos"));
    }

    #[test]
    fn limpeza_semanal_so_quando_ligada_e_vencida() {
        let mut l = LimpezaProgramada::default();
        assert!(!hora_da_limpeza(&l, SEMANA_MS * 10));
        l.cache_de_build = true;
        assert!(hora_da_limpeza(&l, 1));
        l.ultima_ms = Some(1000);
        assert!(!hora_da_limpeza(&l, 1000 + SEMANA_MS - 1));
        assert!(hora_da_limpeza(&l, 1000 + SEMANA_MS));
        let so_tmp = LimpezaProgramada { tmp_dias: Some(14), ..Default::default() };
        assert!(hora_da_limpeza(&so_tmp, 1));
    }

    #[test]
    fn gb_com_virgula() {
        assert_eq!(gb(12 * GIB), "12,0");
        assert_eq!(gb(2_899_102_924), "2,7");
    }
}
