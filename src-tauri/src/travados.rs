//! Processos que parecem presos, a partir das amostras que o agente já manda.
//!
//! O Linux não marca processo como travado; o que dá para ver são sinais. Estado `D`
//! (esperando disco ou rede sem poder ser interrompido) e zumbi que não some são sinais
//! firmes. Um núcleo inteiro por minutos é só suspeita: build e compilação fazem o mesmo.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use protocolo::Amostra;
use serde::Serialize;

const ESPERA_PRESA: Duration = Duration::from_secs(60);
const ZUMBI_PARADO: Duration = Duration::from_secs(60);
const CPU_CONSTANTE: Duration = Duration::from_secs(5 * 60);
/// Fração de um núcleo que já conta como núcleo inteiro, para a medição oscilar sem zerar.
const NUCLEO_CHEIO: f32 = 0.9;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Motivo {
    AguardandoEs,
    Zumbi,
    CpuConstante,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Suspeito {
    pub pid: i32,
    pub ppid: i32,
    pub nome: String,
    pub motivo: Motivo,
    /// Desde quando o sinal aparece, em milissegundos Unix.
    pub desde_ms: u64,
}

struct Visto {
    estado: char,
    estado_desde: Instant,
    cpu_desde: Option<Instant>,
}

/// Um por distro: cada uma tem o próprio namespace de PID.
#[derive(Default)]
pub struct Vigia {
    vistos: HashMap<(i32, String), Visto>,
    ultimos: Vec<Suspeito>,
}

pub struct Leitura {
    pub suspeitos: Vec<Suspeito>,
    /// Os que apareceram nesta amostra, para registrar uma vez só.
    pub novos: Vec<Suspeito>,
    pub mudou: bool,
}

impl Vigia {
    pub fn observar(&mut self, amostra: &Amostra, agora: Instant) -> Leitura {
        let nucleo = 100.0 / amostra.cpus.max(1) as f32;
        let mut presentes = HashSet::new();
        let mut suspeitos = Vec::new();
        for p in &amostra.processos {
            let chave = (p.pid, p.nome.clone());
            presentes.insert(chave.clone());
            let visto = self.vistos.entry(chave).or_insert(Visto { estado: p.estado, estado_desde: agora, cpu_desde: None });
            if visto.estado != p.estado {
                visto.estado = p.estado;
                visto.estado_desde = agora;
            }
            if p.cpu_pct >= nucleo * NUCLEO_CHEIO {
                visto.cpu_desde.get_or_insert(agora);
            } else {
                visto.cpu_desde = None;
            }
            let no_estado = agora.duration_since(visto.estado_desde);
            let motivo = match p.estado {
                'D' if no_estado >= ESPERA_PRESA => Some((Motivo::AguardandoEs, visto.estado_desde)),
                'Z' if no_estado >= ZUMBI_PARADO => Some((Motivo::Zumbi, visto.estado_desde)),
                _ => visto
                    .cpu_desde
                    .filter(|t| agora.duration_since(*t) >= CPU_CONSTANTE)
                    .map(|t| (Motivo::CpuConstante, t)),
            };
            if let Some((motivo, desde)) = motivo {
                suspeitos.push(Suspeito { pid: p.pid, ppid: p.ppid, nome: p.nome.clone(), motivo, desde_ms: em_ms(desde, agora) });
            }
        }
        self.vistos.retain(|chave, _| presentes.contains(chave));

        let antes: HashSet<(i32, Motivo)> = self.ultimos.iter().map(|s| (s.pid, s.motivo)).collect();
        let novos: Vec<Suspeito> = suspeitos.iter().filter(|s| !antes.contains(&(s.pid, s.motivo))).cloned().collect();
        let mudou = !novos.is_empty() || suspeitos.len() != self.ultimos.len();
        self.ultimos = suspeitos.clone();
        Leitura { suspeitos, novos, mudou }
    }
}

/// O `Instant` não tem relação com o relógio de parede; a conversão parte do agora.
fn em_ms(instante: Instant, agora: Instant) -> u64 {
    let ms = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    ms.saturating_sub(agora.duration_since(instante).as_millis() as u64)
}

pub fn descrever(s: &Suspeito) -> String {
    match s.motivo {
        Motivo::AguardandoEs => format!(
            "{} (PID {}) está esperando disco ou rede há mais de 1 min; encerrar só funciona quando a espera terminar",
            s.nome, s.pid
        ),
        Motivo::Zumbi => format!(
            "{} (PID {}) terminou, mas o processo pai (PID {}) não o recolheu; é o pai que precisa ser encerrado",
            s.nome, s.pid, s.ppid
        ),
        Motivo::CpuConstante => format!("{} (PID {}) usa um núcleo inteiro há mais de 5 min", s.nome, s.pid),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use protocolo::{CustoAgente, Processo};

    fn amostra(processos: Vec<(i32, char, f32)>) -> Amostra {
        Amostra {
            seq: 1,
            uptime_s: 0.0,
            cpus: 4,
            cpu_vm_pct: 0.0,
            mem_total: 0,
            mem_disponivel: 0,
            mem_livre: 0,
            mem_blocos: 0,
            mem_arquivos: 0,
            mem_tabelas: 0,
            swap_total: 0,
            swap_livre: 0,
            processos: processos
                .into_iter()
                .map(|(pid, estado, cpu_pct)| Processo {
                    pid,
                    ppid: 1,
                    usuario: "root".into(),
                    estado,
                    nome: format!("p{pid}"),
                    cmdline: String::new(),
                    cpu_pct,
                    rss: 0,
                })
                .collect(),
            portas: None,
            kernel: Vec::new(),
            agente: CustoAgente::default(),
        }
    }

    #[test]
    fn espera_presa_so_depois_de_um_minuto_seguido() {
        let mut v = Vigia::default();
        let t0 = Instant::now();
        assert!(v.observar(&amostra(vec![(10, 'D', 0.0)]), t0).suspeitos.is_empty());
        assert!(v.observar(&amostra(vec![(10, 'D', 0.0)]), t0 + Duration::from_secs(59)).suspeitos.is_empty());
        let l = v.observar(&amostra(vec![(10, 'D', 0.0)]), t0 + Duration::from_secs(60));
        assert_eq!(l.novos.len(), 1);
        assert_eq!(l.suspeitos[0].motivo, Motivo::AguardandoEs);
        // Já registrado: não volta como novo na amostra seguinte.
        let l = v.observar(&amostra(vec![(10, 'D', 0.0)]), t0 + Duration::from_secs(61));
        assert!(l.novos.is_empty() && !l.mudou);
        // Saiu do estado D: sai da lista, e a contagem recomeça se voltar.
        let l = v.observar(&amostra(vec![(10, 'S', 0.0)]), t0 + Duration::from_secs(62));
        assert!(l.suspeitos.is_empty() && l.mudou);
        assert!(v.observar(&amostra(vec![(10, 'D', 0.0)]), t0 + Duration::from_secs(63)).suspeitos.is_empty());
    }

    #[test]
    fn cpu_conta_nucleo_inteiro_pelo_total_de_cpus() {
        let mut v = Vigia::default();
        let t0 = Instant::now();
        // 4 CPUs: um núcleo é 25% do total; 23% ainda conta, 20% não.
        v.observar(&amostra(vec![(20, 'R', 23.0), (21, 'R', 20.0)]), t0);
        let l = v.observar(&amostra(vec![(20, 'R', 24.0), (21, 'R', 20.0)]), t0 + CPU_CONSTANTE);
        assert_eq!(l.suspeitos.len(), 1);
        assert_eq!((l.suspeitos[0].pid, l.suspeitos[0].motivo), (20, Motivo::CpuConstante));
        // Uma amostra abaixo do limiar zera a contagem.
        v.observar(&amostra(vec![(20, 'S', 1.0)]), t0 + CPU_CONSTANTE + Duration::from_secs(1));
        let l = v.observar(&amostra(vec![(20, 'R', 25.0)]), t0 + CPU_CONSTANTE + Duration::from_secs(2));
        assert!(l.suspeitos.is_empty());
    }

    #[test]
    fn zumbi_aponta_o_pai() {
        let mut v = Vigia::default();
        let t0 = Instant::now();
        v.observar(&amostra(vec![(30, 'Z', 0.0)]), t0);
        let l = v.observar(&amostra(vec![(30, 'Z', 0.0)]), t0 + ZUMBI_PARADO);
        assert_eq!(l.suspeitos[0].motivo, Motivo::Zumbi);
        assert!(descrever(&l.suspeitos[0]).contains("PID 1"));
    }

    #[test]
    fn processo_que_sumiu_sai_da_memoria() {
        let mut v = Vigia::default();
        let t0 = Instant::now();
        v.observar(&amostra(vec![(40, 'D', 0.0)]), t0);
        v.observar(&amostra(vec![]), t0 + Duration::from_secs(1));
        assert!(v.vistos.is_empty());
    }
}
