//! Memória do Windows e o que a VM do WSL ocupa nela.
//!
//! Medido numa máquina de 32 GB: com o Windows a 2,7 GB livres e a VM no teto do
//! `.wslconfig`, o WSL parou de responder (`Wsl/Service/0x8007274c`) e só voltou com
//! `wsl --shutdown`. O app acompanha a memória do Windows para avisar antes disso, e
//! mostra quem dentro da VM ocupa mais, por projeto do Docker.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use log::{info, warn};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use crate::agentes::Agentes;
use crate::docker::{self, Conteiner};

const PAUSA: Duration = Duration::from_millis(500);
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

#[derive(Default)]
pub struct Recursos {
    pub memoria: Mutex<MemoriaWindows>,
    pub docker: Mutex<Docker>,
    /// Pedido de nova leitura do Docker logo depois de parar ou ligar containers.
    atualizar_docker: AtomicBool,
}

impl Recursos {
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
        wsl_bytes: memoria_fisica_de("vmmemWSL"),
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

/// O que mais pesa dentro da VM. Quando o cache passa da memória somada dos containers,
/// apontar o maior container manda olhar para o lugar errado: medido numa máquina, a VM
/// tinha 10,1 GB de cache e o maior container, 219 MB.
pub fn explicar_wsl(wsl: u64, cache: Option<u64>, conteineres: &[Conteiner]) -> String {
    let docker: u64 = conteineres.iter().map(|c| c.memoria_bytes).sum();
    match (cache, maior_consumidor(conteineres)) {
        (Some(cache), _) if cache >= docker => format!(
            "O WSL ocupa {} GB, e a maior parte é cache de arquivos ({} GB), que o Linux devolve quando precisa mas o Windows conta como ocupado. Baixar o limite de memória do WSL, em Desempenho, segura esse tamanho.",
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
        loop {
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

fn avisar_pressao(app: &AppHandle, agentes: &Agentes, recursos: &Recursos, m: &MemoriaWindows) {
    if !m.pressao {
        info!("memória do Windows voltou: {} GB disponíveis", gb(m.disponivel_bytes));
        agentes.registrar(app, "memoria", None, format!("A memória do Windows voltou a ter folga: {} GB disponíveis", gb(m.disponivel_bytes)));
        return;
    }
    let mut texto = format!("{} GB livres de {} GB.", gb(m.disponivel_bytes), gb(m.total_bytes));
    if let Some(wsl) = m.wsl_bytes {
        texto.push(' ');
        texto.push_str(&explicar_wsl(wsl, agentes.cache_vm(), &recursos.docker.lock().unwrap().conteineres));
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
        assert!(explicar_wsl(8 * GIB, Some(10 * GIB), &runner).contains("cache de arquivos (10,0 GB)"));
        let loja = [c("loja-db", 4 * 1024)];
        assert!(explicar_wsl(8 * GIB, Some(2 * GIB), &loja).contains("quem mais usa dentro dele é loja-db"));
        assert_eq!(explicar_wsl(8 * GIB, None, &[]), "O WSL ocupa 8,0 GB.");
    }

    #[test]
    fn gb_com_virgula() {
        assert_eq!(gb(12 * GIB), "12,0");
        assert_eq!(gb(2_899_102_924), "2,7");
    }
}
