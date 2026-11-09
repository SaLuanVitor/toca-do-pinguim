//! Pressão da VM e quem a causa. O kernel mede, em `/proc/pressure`, quanto do tempo os
//! processos passaram esperando CPU, memória ou disco; "full" é o tempo em que todos esperavam
//! e a VM não andava. É o sinal mais cedo de que ela está afogada: nas travas de 08/11/2026 o
//! disco já prendia processos (`php` em estado D) antes de o Docker e o WSL pararem.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use protocolo::{Pressao, Processo};
use serde::{Deserialize, Serialize};

/// Limiar inicial, uma decisão e não uma medição: a trava de 08/11 aconteceu antes de o app ler
/// a pressão. Fora dela, no mesmo dia, o disco teve "full" de 5% na média de 60 s. O log
/// registra toda leitura acima da metade do limiar, para calibrar.
pub const LIMIAR: f32 = 10.0;
/// CPU da VM, em % do total. Para a CPU o kernel não mede o "full" do sistema (fica em zero), e
/// o sinal é o uso dela. Também é limiar inicial, a calibrar pelo log.
pub const LIMIAR_CPU: f32 = 90.0;
/// O limiar precisa durar isto antes do aviso: um pico curto de disco não é afogamento.
const DURACAO: Duration = Duration::from_secs(120);
/// Uma linha de log por minuto, no máximo, com a pressão acima da metade do limiar.
const LOG_A_CADA: Duration = Duration::from_secs(60);
/// Grupos que entram na lista de quem pesa.
const MAIS_PESADOS: usize = 5;
const GIB: u64 = 1 << 30;
const MIB: u64 = 1 << 20;
/// Abaixo disto, ler e gravar no disco não pesa: entra na lista quem passa de 5 MB/s.
const DISCO_PESADO: u64 = 5 * MIB;

/// Processos de mesmo nome num container (ou fora dele) de uma distro, somados.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pesado {
    pub distro: String,
    pub nome: String,
    /// Id do container dono dos processos, quando são de um.
    #[serde(default)]
    pub conteiner: Option<String>,
    /// Os processos somados, para baixar a prioridade deles; no máximo 20.
    #[serde(default)]
    pub pids: Vec<i32>,
    pub quantos: u32,
    /// Em núcleos: 1,0 é um núcleo inteiro.
    pub cpu_nucleos: f32,
    pub rss: u64,
    /// Quantos estão em estado D, esperando disco ou rede.
    pub esperando: u32,
    /// Leitura e gravação no disco, em bytes por segundo.
    pub disco_bps: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct AvisoPressao {
    pub desde_ms: u64,
    pub disco_full: f32,
    pub memoria_full: f32,
    pub cpu_some: f32,
    /// Uso da CPU da VM, em % do total.
    pub cpu_vm: f32,
    pub pesados: Vec<Pesado>,
}

/// Agrupa os processos por distro e nome e escolhe os que mais pesam: os que mais usam CPU
/// (meio núcleo ou mais), os que esperam disco e o maior em memória, quando passa de 1 GB.
pub fn escolher(distros: &[(String, u32, Vec<Processo>)]) -> Vec<Pesado> {
    let mut grupos: HashMap<(String, Option<String>, String), Pesado> = HashMap::new();
    for (distro, cpus, processos) in distros {
        for p in processos {
            let g = grupos.entry((distro.clone(), p.conteiner.clone(), p.nome.clone())).or_insert_with(|| Pesado {
                distro: distro.clone(),
                nome: p.nome.clone(),
                conteiner: p.conteiner.clone(),
                pids: Vec::new(),
                quantos: 0,
                cpu_nucleos: 0.0,
                rss: 0,
                esperando: 0,
                disco_bps: 0,
            });
            g.quantos += 1;
            if g.pids.len() < 20 {
                g.pids.push(p.pid);
            }
            g.cpu_nucleos += p.cpu_pct * *cpus as f32 / 100.0;
            g.rss += p.rss;
            g.disco_bps += p.disco_bps;
            if p.estado == 'D' {
                g.esperando += 1;
            }
        }
    }
    let mut todos: Vec<Pesado> = grupos.into_values().collect();
    todos.sort_by(|a, b| b.cpu_nucleos.total_cmp(&a.cpu_nucleos).then_with(|| a.nome.cmp(&b.nome)));
    let mut escolhidos: Vec<Pesado> = todos.iter().filter(|g| g.cpu_nucleos >= 0.5).take(3).cloned().collect();
    let mut esperando: Vec<&Pesado> = todos.iter().filter(|g| g.esperando > 0).collect();
    esperando.sort_by_key(|g| std::cmp::Reverse(g.esperando));
    let mut no_disco: Vec<&Pesado> = todos.iter().filter(|g| g.disco_bps >= DISCO_PESADO).collect();
    no_disco.sort_by_key(|g| std::cmp::Reverse(g.disco_bps));
    let mais_memoria = todos.iter().filter(|g| g.rss >= GIB).max_by_key(|g| g.rss);
    for g in no_disco.into_iter().take(2).chain(esperando.into_iter().take(2)).chain(mais_memoria) {
        if !escolhidos.iter().any(|e| e.distro == g.distro && e.conteiner == g.conteiner && e.nome == g.nome) {
            escolhidos.push(g.clone());
        }
    }
    escolhidos.truncate(MAIS_PESADOS);
    escolhidos
}

fn numero(v: f32) -> String {
    format!("{v:.1}").replace('.', ",")
}

/// "sqlservr em docker-desktop (1,0 núcleo, 2,1 GB); 6 php em docker-desktop esperando disco".
pub fn descrever(pesados: &[Pesado]) -> String {
    pesados
        .iter()
        .map(|p| {
            let nome = if p.quantos > 1 { format!("{} {}", p.quantos, p.nome) } else { p.nome.clone() };
            let mut partes = Vec::new();
            if p.cpu_nucleos >= 0.5 {
                let n = numero(p.cpu_nucleos);
                partes.push(if p.cpu_nucleos < 1.5 { format!("{n} núcleo") } else { format!("{n} núcleos") });
            }
            if p.rss >= GIB {
                partes.push(format!("{} GB", numero(p.rss as f32 / GIB as f32)));
            }
            if p.disco_bps >= DISCO_PESADO {
                partes.push(format!("{} MB/s de disco", numero(p.disco_bps as f32 / MIB as f32)));
            }
            if p.esperando > 0 {
                partes.push(if p.esperando == p.quantos { "esperando disco".to_string() } else { format!("{} esperando disco", p.esperando) });
            }
            format!("{nome} em {} ({})", p.distro, partes.join(", "))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub enum Mudanca {
    Nada,
    /// Passou do limiar pelo tempo combinado: hora de avisar.
    Avisar,
    /// Voltou para abaixo da metade do limiar depois de um aviso.
    Voltou,
}

/// Quanto a leitura está do limiar: 1 é chegar nele, no pior dos três (disco, memória, CPU).
pub fn nivel(p: &Pressao, cpu_vm: f32) -> f32 {
    (p.disco_full.avg60 / LIMIAR).max(p.memoria_full.avg60 / LIMIAR).max(cpu_vm / LIMIAR_CPU)
}

/// Se o que mais pesa é a CPU, e não a espera por disco ou memória.
pub fn pela_cpu(p: &Pressao, cpu_vm: f32) -> bool {
    cpu_vm / LIMIAR_CPU >= p.disco_full.avg60.max(p.memoria_full.avg60) / LIMIAR
}

/// Acompanha a pressão a cada leitura e diz quando avisar e quando o aviso acaba. A volta pede
/// metade do limiar, para o aviso não piscar em torno dele.
#[derive(Default)]
pub struct Vigia {
    desde: Option<Instant>,
    pub desde_ms: u64,
    pub avisando: bool,
    ultimo_log: Option<Instant>,
}

impl Vigia {
    pub fn avaliar(&mut self, p: &Pressao, cpu_vm: f32, agora: Instant, agora_ms: u64) -> Mudanca {
        let nivel = nivel(p, cpu_vm);
        let vale_log = p.disco_full.avg60.max(p.memoria_full.avg60) >= LIMIAR / 2.0 || cpu_vm >= LIMIAR_CPU;
        if vale_log && self.ultimo_log.is_none_or(|t| agora.duration_since(t) >= LOG_A_CADA) {
            self.ultimo_log = Some(agora);
            log::info!(
                "pressão da VM (média de 60 s): disco full {:.1}%, memória full {:.1}%, CPU some {:.1}%; CPU da VM {:.0}%",
                p.disco_full.avg60,
                p.memoria_full.avg60,
                p.cpu_some.avg60,
                cpu_vm
            );
        }
        if nivel >= 1.0 {
            let desde = *self.desde.get_or_insert_with(|| {
                self.desde_ms = agora_ms;
                agora
            });
            if !self.avisando && agora.duration_since(desde) >= DURACAO {
                self.avisando = true;
                return Mudanca::Avisar;
            }
        } else if nivel < 0.5 {
            self.desde = None;
            if self.avisando {
                self.avisando = false;
                return Mudanca::Voltou;
            }
        }
        Mudanca::Nada
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use protocolo::Espera;

    fn processo(nome: &str, estado: char, cpu_pct: f32, rss: u64) -> Processo {
        Processo { pid: 1, ppid: 0, usuario: "root".into(), estado, nome: nome.into(), cmdline: String::new(), cpu_pct, rss, nice: 0, disco_bps: 0, conteiner: None }
    }

    #[test]
    fn quem_mais_usa_o_disco_entra_primeiro_entre_os_sem_cpu() {
        let mut runner = processo("gitlab-runner-helper", 'R', 0.2, 0);
        runner.disco_bps = 40 * MIB;
        let mut pouco = processo("cron", 'S', 0.0, 0);
        pouco.disco_bps = MIB;
        let p = escolher(&[("docker-desktop".into(), 10, vec![runner, pouco, processo("php", 'D', 0.0, 0)])]);
        assert_eq!(descrever(&p), "gitlab-runner-helper em docker-desktop (40,0 MB/s de disco); php em docker-desktop (esperando disco)");
    }

    #[test]
    fn escolhe_cpu_quem_espera_disco_e_o_maior_em_memoria() {
        let docker = vec![
            processo("sqlservr", 'S', 10.0, 3 * GIB),
            processo("sqlservr", 'R', 9.8, GIB / 2),
            processo("php", 'D', 0.1, 0),
            processo("php", 'D', 0.1, 0),
            processo("supervisord", 'D', 0.0, 0),
            processo("nginx", 'S', 0.2, 0),
        ];
        let ubuntu = vec![processo("node", 'S', 1.0, 2 * GIB)];
        let p = escolher(&[("docker-desktop".into(), 10, docker), ("Ubuntu".into(), 10, ubuntu)]);
        let nomes: Vec<&str> = p.iter().map(|g| g.nome.as_str()).collect();
        assert_eq!(nomes, ["sqlservr", "php", "supervisord"]);
        assert_eq!(p[0].quantos, 2);
        assert!((p[0].cpu_nucleos - 1.98).abs() < 0.01);
        assert_eq!(p[1].esperando, 2);
        assert_eq!(
            descrever(&p),
            "2 sqlservr em docker-desktop (2,0 núcleos, 3,5 GB); 2 php em docker-desktop (esperando disco); supervisord em docker-desktop (esperando disco)"
        );
    }

    #[test]
    fn maior_em_memoria_entra_mesmo_sem_cpu() {
        let p = escolher(&[("Ubuntu".into(), 4, vec![processo("java", 'S', 1.0, 5 * GIB), processo("bash", 'S', 0.0, 0)])]);
        assert_eq!(descrever(&p), "java em Ubuntu (5,0 GB)");
    }

    fn pressao(disco_full: f32) -> Pressao {
        Pressao { disco_full: Espera { avg10: disco_full, avg60: disco_full }, ..Default::default() }
    }

    #[test]
    fn avisa_depois_de_dois_minutos_e_volta_abaixo_da_metade() {
        let mut v = Vigia::default();
        let t0 = Instant::now();
        let em = |s: u64| t0 + Duration::from_secs(s);
        assert!(matches!(v.avaliar(&pressao(12.0), 0.0, em(0), 0), Mudanca::Nada));
        assert!(matches!(v.avaliar(&pressao(12.0), 0.0, em(119), 0), Mudanca::Nada));
        assert!(matches!(v.avaliar(&pressao(12.0), 0.0, em(120), 0), Mudanca::Avisar));
        assert!(matches!(v.avaliar(&pressao(12.0), 0.0, em(200), 0), Mudanca::Nada));
        // Entre a metade e o limiar o aviso fica: sem piscar.
        assert!(matches!(v.avaliar(&pressao(7.0), 0.0, em(210), 0), Mudanca::Nada));
        assert!(v.avisando);
        assert!(matches!(v.avaliar(&pressao(4.0), 0.0, em(220), 0), Mudanca::Voltou));
        assert!(!v.avisando);
    }

    #[test]
    fn pico_curto_nao_avisa() {
        let mut v = Vigia::default();
        let t0 = Instant::now();
        let em = |s: u64| t0 + Duration::from_secs(s);
        v.avaliar(&pressao(30.0), 0.0, em(0), 0);
        v.avaliar(&pressao(3.0), 0.0, em(60), 0);
        assert!(matches!(v.avaliar(&pressao(30.0), 0.0, em(100), 0), Mudanca::Nada));
        assert!(matches!(v.avaliar(&pressao(30.0), 0.0, em(219), 0), Mudanca::Nada));
    }

    #[test]
    fn cpu_cheia_tambem_avisa_e_diz_que_e_a_cpu() {
        let mut v = Vigia::default();
        let t0 = Instant::now();
        let em = |s: u64| t0 + Duration::from_secs(s);
        let calmo = pressao(1.0);
        assert!(matches!(v.avaliar(&calmo, 95.0, em(0), 0), Mudanca::Nada));
        assert!(matches!(v.avaliar(&calmo, 92.0, em(120), 0), Mudanca::Avisar));
        assert!(pela_cpu(&calmo, 92.0));
        assert!(!pela_cpu(&pressao(30.0), 92.0));
        // 60% está acima da metade (45%): o aviso fica.
        assert!(matches!(v.avaliar(&calmo, 60.0, em(130), 0), Mudanca::Nada));
        assert!(matches!(v.avaliar(&calmo, 40.0, em(140), 0), Mudanca::Voltou));
    }
}
