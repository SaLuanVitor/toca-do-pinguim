//! Agente que roda dentro de uma distro e transmite o estado dela pelo stdout.
//!
//! Vive enquanto o app vive: entrada padrão fechada ou saída padrão quebrada encerram o
//! processo, para que o agente nunca segure a distro ligada depois que a janela fecha.
//!
//! `--uma-vez` coleta uma amostra e sai, para comparar com a consulta repetida.
//! `--sair-se-ocioso` encerra o agente quando a sessão dele é a única aberta na distro,
//! para não ser ele o motivo de ela continuar ligada.

mod kernel;
mod portas;

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Write};
use std::os::unix::fs::MetadataExt;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use protocolo::{Amostra, Comando, CustoAgente, Mensagem, Processo};

const CMDLINE_MAX: usize = 512;

/// Varrer os descritores de todos os processos custa mais que o resto da amostra junto.
const PORTAS_A_CADA: u64 = 5;

/// Amostras seguidas sem atividade antes de sair. Uma só pegaria o instante entre um
/// processo terminar e o seguinte começar, num script por exemplo.
const AMOSTRAS_OCIOSAS: u32 = 2;

struct CpuVm {
    ocupado: u64,
    total: u64,
}

/// Leitura bruta de um processo. A chave inclui o `starttime` para que um PID
/// reaproveitado não herde a taxa do processo que morreu.
struct Bruto {
    pid: i32,
    ppid: i32,
    estado: char,
    nome: String,
    ticks: u64,
    inicio: u64,
    rss_paginas: u64,
    uid: u32,
}

struct Coletor {
    tick_hz: f64,
    pagina: u64,
    usuarios: HashMap<u32, String>,
    anterior_uptime: Option<f64>,
    anterior_cpu: Option<CpuVm>,
    anterior_proc: HashMap<(i32, u64), u64>,
    /// A linha de comando só muda num `exec`, que troca o nome do processo; com o nome na
    /// chave, cada processo tem o `cmdline` lido uma vez só.
    cmdlines: HashMap<(i32, u64, String), String>,
    anterior_self: Option<u64>,
    kmsg: Option<kernel::Kmsg>,
    seq: u64,
}

impl Coletor {
    fn novo() -> Self {
        // SAFETY: sysconf só lê constantes do sistema.
        let tick_hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as f64;
        let pagina = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(1) as u64;
        Coletor {
            tick_hz,
            pagina,
            usuarios: ler_usuarios(),
            anterior_uptime: None,
            anterior_cpu: None,
            anterior_proc: HashMap::new(),
            cmdlines: HashMap::new(),
            anterior_self: None,
            kmsg: kernel::Kmsg::abrir(),
            seq: 0,
        }
    }

    fn coletar(&mut self) -> Amostra {
        let comeco = Instant::now();
        let uptime = ler_uptime();
        let (cpu, cpus) = ler_cpu_vm();
        let mem = ler_meminfo();
        let brutos = ler_processos();

        let dt = self.anterior_uptime.map(|a| uptime - a).filter(|d| *d > 0.0);
        let cpu_vm_pct = match (&self.anterior_cpu, dt) {
            (Some(a), Some(_)) if cpu.total > a.total => {
                100.0 * (cpu.ocupado - a.ocupado) as f32 / (cpu.total - a.total) as f32
            }
            _ => 0.0,
        };

        let capacidade = dt.map(|d| d * self.tick_hz * cpus as f64);
        let mut atual_proc = HashMap::with_capacity(brutos.len());
        let mut cmdlines = HashMap::with_capacity(brutos.len());
        let processos = brutos
            .into_iter()
            .map(|b| {
                let chave = (b.pid, b.inicio);
                let cpu_pct = match (self.anterior_proc.get(&chave), capacidade) {
                    (Some(antes), Some(cap)) if b.ticks >= *antes => {
                        (100.0 * (b.ticks - antes) as f64 / cap) as f32
                    }
                    _ => 0.0,
                };
                atual_proc.insert(chave, b.ticks);
                let chave_cmd = (b.pid, b.inicio, b.nome.clone());
                let cmdline = self
                    .cmdlines
                    .remove(&chave_cmd)
                    .unwrap_or_else(|| ler_cmdline(b.pid));
                cmdlines.insert(chave_cmd, cmdline.clone());
                Processo {
                    pid: b.pid,
                    ppid: b.ppid,
                    usuario: self.usuarios.get(&b.uid).cloned().unwrap_or_else(|| b.uid.to_string()),
                    estado: b.estado,
                    cmdline,
                    nome: b.nome,
                    cpu_pct,
                    rss: b.rss_paginas * self.pagina,
                }
            })
            .collect();

        let portas = (self.seq % PORTAS_A_CADA == 0).then(portas::coletar);
        let kernel = self.kmsg.as_mut().map(kernel::Kmsg::novos).unwrap_or_default();

        let (ticks_self, rss_self) = ler_self();
        let cpu_self = match (self.anterior_self, capacidade) {
            (Some(antes), Some(cap)) => (100.0 * ticks_self.saturating_sub(antes) as f64 / cap) as f32,
            _ => 0.0,
        };

        self.anterior_uptime = Some(uptime);
        self.anterior_cpu = Some(cpu);
        self.anterior_proc = atual_proc;
        self.cmdlines = cmdlines;
        self.anterior_self = Some(ticks_self);
        self.seq += 1;

        Amostra {
            seq: self.seq,
            uptime_s: uptime,
            cpus,
            cpu_vm_pct,
            mem_total: mem.get("MemTotal").copied().unwrap_or(0),
            mem_disponivel: mem.get("MemAvailable").copied().unwrap_or(0),
            swap_total: mem.get("SwapTotal").copied().unwrap_or(0),
            swap_livre: mem.get("SwapFree").copied().unwrap_or(0),
            processos,
            portas,
            kernel,
            agente: CustoAgente {
                rss: rss_self * self.pagina,
                cpu_pct: cpu_self,
                coleta_us: comeco.elapsed().as_micros() as u64,
            },
        }
    }
}

fn ler_uptime() -> f64 {
    fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse().ok())
        .unwrap_or(0.0)
}

fn ler_cpu_vm() -> (CpuVm, u32) {
    let texto = fs::read_to_string("/proc/stat").unwrap_or_default();
    let mut cpus = 0;
    let mut vm = CpuVm { ocupado: 0, total: 0 };
    for linha in texto.lines() {
        if let Some(resto) = linha.strip_prefix("cpu ") {
            let v: Vec<u64> = resto.split_whitespace().filter_map(|c| c.parse().ok()).collect();
            // user nice system idle iowait irq softirq steal; guest já está contado em user.
            let total: u64 = v.iter().take(8).sum();
            let ocioso = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
            vm = CpuVm { ocupado: total - ocioso, total };
        } else if linha.starts_with("cpu") {
            cpus += 1;
        }
    }
    (vm, cpus.max(1))
}

fn ler_meminfo() -> HashMap<String, u64> {
    let texto = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    texto
        .lines()
        .filter_map(|l| {
            let (chave, resto) = l.split_once(':')?;
            // Os valores de /proc/meminfo vêm em KiB, qualquer que seja o rótulo.
            let kib: u64 = resto.split_whitespace().next()?.parse().ok()?;
            Some((chave.to_string(), kib * 1024))
        })
        .collect()
}

fn ler_processos() -> Vec<Bruto> {
    let Ok(dir) = fs::read_dir("/proc") else { return Vec::new() };
    dir.filter_map(|e| e.ok()?.file_name().to_str()?.parse::<i32>().ok())
        .filter_map(ler_processo)
        .collect()
}

/// Processo que morre entre o `read_dir` e a leitura some da amostra em silêncio.
fn ler_processo(pid: i32) -> Option<Bruto> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // O nome pode conter ')' e espaços; só o último ')' marca o fim dele.
    let abre = stat.find('(')?;
    let fecha = stat.rfind(')')?;
    let nome = stat[abre + 1..fecha].to_string();
    let f: Vec<&str> = stat[fecha + 1..].split_whitespace().collect();
    let num = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    // O dono de /proc/<pid> é o UID efetivo do processo; ler o metadado evita abrir o
    // arquivo `status` de cada processo a cada amostra.
    let uid = fs::metadata(format!("/proc/{pid}")).map(|m| m.uid()).unwrap_or(u32::MAX);
    Some(Bruto {
        pid,
        ppid: f.get(1)?.parse().ok()?,
        estado: f.first()?.chars().next()?,
        nome,
        ticks: num(11) + num(12),
        inicio: num(19),
        rss_paginas: num(21),
        uid,
    })
}

fn ler_cmdline(pid: i32) -> String {
    let bytes = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    let mut s = String::from_utf8_lossy(&bytes).replace('\0', " ");
    s.truncate(s.trim_end().len());
    if s.len() > CMDLINE_MAX {
        let corte = (0..=CMDLINE_MAX).rev().find(|i| s.is_char_boundary(*i)).unwrap_or(0);
        s.truncate(corte);
    }
    s
}

fn ler_self() -> (u64, u64) {
    let stat = fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let Some(fecha) = stat.rfind(')') else { return (0, 0) };
    let f: Vec<u64> = stat[fecha + 1..]
        .split_whitespace()
        .map(|v| v.parse().unwrap_or(0))
        .collect();
    let num = |i: usize| f.get(i).copied().unwrap_or(0);
    (num(11) + num(12), num(21))
}

fn ler_usuarios() -> HashMap<u32, String> {
    fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut c = l.split(':');
            let nome = c.next()?;
            let uid = c.nth(1)?.parse().ok()?;
            Some((uid, nome.to_string()))
        })
        .collect()
}

fn enviar_sinal(pid: i32, sinal: i32) -> Result<(), String> {
    // PID 1 é o init da distro: derrubá-lo encerra a distro inteira, e isso é papel do
    // `wsl --terminate`, não de um kill na lista de processos.
    if pid <= 1 {
        return Err(format!("PID {pid} é protegido"));
    }
    if pid as u32 == std::process::id() {
        return Err("o agente não encerra a si mesmo por sinal; use o comando sair".into());
    }
    // SAFETY: kill não toca memória do processo; o retorno é conferido logo abaixo.
    if unsafe { libc::kill(pid, sinal) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error().to_string())
    }
}

/// O WSL desliga uma distro ociosa quando não sobra sessão aberta nela. Medido no WSL 2.6.1
/// com uma Debian com systemd: com `cron`, `dbus`, `journald` e `udevd` rodando, ela
/// desligou 16 s depois que a última sessão fechou. Cada `wsl.exe` aberto aparece lá
/// dentro como um `SessionLeader` filho do init do WSL, e é isso que o agente conta.
///
/// A sessão do próprio agente fica de fora: é o `SessionLeader` acima do `Relay` que o
/// lançou.
fn sem_outras_sessoes(amostra: &Amostra) -> bool {
    let eu = std::process::id() as i32;
    let pai = |pid: i32| amostra.processos.iter().find(|p| p.pid == pid).map(|p| p.ppid);
    let minha_sessao = pai(eu).and_then(pai);
    !amostra
        .processos
        .iter()
        .any(|p| p.nome == "SessionLeader" && Some(p.pid) != minha_sessao)
}

fn escrever(saida: &mut impl Write, msg: &Mensagem) -> io::Result<()> {
    serde_json::to_writer(&mut *saida, msg)?;
    saida.write_all(b"\n")?;
    saida.flush()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut intervalo = args
        .iter()
        .position(|a| a == "--intervalo")
        .and_then(|i| args.get(i + 1)?.parse().ok())
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_millis(1000));
    let uma_vez = args.iter().any(|a| a == "--uma-vez");
    let sair_se_ocioso = args.iter().any(|a| a == "--sair-se-ocioso");

    let mut coletor = Coletor::novo();
    let mut saida = io::stdout().lock();

    if uma_vez {
        let _ = escrever(&mut saida, &Mensagem::Amostra(coletor.coletar()));
        return;
    }

    let (tx, rx) = mpsc::channel::<Option<Comando>>();
    thread::spawn(move || {
        for linha in io::stdin().lock().lines() {
            let Ok(linha) = linha else { break };
            // Escritores .NET costumam abrir o fluxo com BOM; ele não é parte do JSON.
            let linha = linha.trim_start_matches('\u{feff}').trim();
            if linha.is_empty() {
                continue;
            }
            match serde_json::from_str::<Comando>(linha) {
                Ok(cmd) => {
                    if tx.send(Some(cmd)).is_err() {
                        return;
                    }
                }
                Err(e) => eprintln!("comando inválido: {e}"),
            }
        }
        // stdin fechado: o app foi embora.
        let _ = tx.send(None);
    });

    let mut ociosas = 0;
    loop {
        let amostra = coletor.coletar();
        ociosas = if sair_se_ocioso && sem_outras_sessoes(&amostra) { ociosas + 1 } else { 0 };
        if escrever(&mut saida, &Mensagem::Amostra(amostra)).is_err() {
            return;
        }
        if ociosas >= AMOSTRAS_OCIOSAS {
            let _ = escrever(&mut saida, &Mensagem::Ocioso);
            return;
        }
        let prazo = Instant::now() + intervalo;
        loop {
            let resta = prazo.saturating_duration_since(Instant::now());
            match rx.recv_timeout(resta) {
                Ok(Some(Comando::Sinal { pid, sinal })) => {
                    let r = enviar_sinal(pid, sinal);
                    let resposta = Mensagem::Resposta { ok: r.is_ok(), erro: r.err() };
                    if escrever(&mut saida, &resposta).is_err() {
                        return;
                    }
                }
                Ok(Some(Comando::Intervalo { ms })) => intervalo = Duration::from_millis(ms.max(100)),
                Ok(Some(Comando::Sair)) | Ok(None) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => break,
            }
        }
    }
}
