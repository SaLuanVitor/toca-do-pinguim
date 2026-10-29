//! Linha do tempo do que aconteceu no WSL: distros que ligaram e desligaram, processos
//! encerrados pelo app, configuração alterada e processos mortos por falta de memória.
//!
//! Fica em disco (uma linha JSON por evento) para sobreviver ao fechamento do app.

use std::collections::{HashSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use protocolo::EventoKernel;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const EVENTOS_GUARDADOS: usize = 1000;
const SEQS_LEMBRADOS: usize = 500;

#[derive(Serialize, Deserialize, Clone)]
pub struct Evento {
    pub quando_ms: u64,
    /// `distro`, `processo`, `memoria`, `config` ou `wsl`.
    pub tipo: String,
    pub distro: Option<String>,
    pub texto: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detalhe: Option<String>,
}

pub struct Atividade {
    arquivo: PathBuf,
    eventos: VecDeque<Evento>,
    kernel_vistos: VecDeque<u64>,
    kernel_set: HashSet<u64>,
}

impl Atividade {
    pub fn abrir(arquivo: PathBuf) -> Self {
        let mut eventos: VecDeque<Evento> = fs::read_to_string(&arquivo)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        while eventos.len() > EVENTOS_GUARDADOS {
            eventos.pop_front();
        }
        let texto: String = eventos
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .map(|l| l + "\n")
            .collect();
        let _ = fs::write(&arquivo, texto);
        Atividade { arquivo, eventos, kernel_vistos: VecDeque::new(), kernel_set: HashSet::new() }
    }

    pub fn listar(&self) -> Vec<Evento> {
        self.eventos.iter().cloned().collect()
    }

    pub fn registrar(
        &mut self,
        app: &AppHandle,
        tipo: &str,
        distro: Option<&str>,
        texto: String,
        detalhe: Option<String>,
    ) {
        let e = Evento {
            quando_ms: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
            tipo: tipo.to_string(),
            distro: distro.map(str::to_string),
            texto,
            detalhe,
        };
        if let (Ok(linha), Ok(mut f)) =
            (serde_json::to_string(&e), OpenOptions::new().create(true).append(true).open(&self.arquivo))
        {
            let _ = writeln!(f, "{linha}");
        }
        let _ = app.emit("atividade", &e);
        self.eventos.push_back(e);
        if self.eventos.len() > EVENTOS_GUARDADOS {
            self.eventos.pop_front();
        }
    }

    /// O log do kernel é um só para a VM e chega repetido pelo agente de cada distro; o
    /// `seq` do registro identifica a repetição.
    pub fn kernel(&mut self, app: &AppHandle, ev: &EventoKernel) {
        if !self.kernel_set.insert(ev.seq) {
            return;
        }
        self.kernel_vistos.push_back(ev.seq);
        if self.kernel_vistos.len() > SEQS_LEMBRADOS {
            if let Some(velho) = self.kernel_vistos.pop_front() {
                self.kernel_set.remove(&velho);
            }
        }
        self.registrar(app, "memoria", None, traduzir_oom(&ev.texto), Some(ev.texto.clone()));
    }

    /// O `seq` do kernel recomeça quando a VM reinicia; com todas as distros paradas, a
    /// próxima VM pode reaproveitar números já vistos.
    pub fn vm_parou(&mut self) {
        self.kernel_vistos.clear();
        self.kernel_set.clear();
    }
}

/// `Out of memory: Killed process 1234 (node) total-vm:...` vira uma frase legível. O PID
/// é o da VM, não o da distro: o kernel registra o número do namespace raiz.
fn traduzir_oom(texto: &str) -> String {
    let nome = texto
        .split_once('(')
        .and_then(|(_, r)| r.split_once(')'))
        .map(|(n, _)| n.to_string());
    match nome {
        Some(n) => format!("Faltou memória na VM e o kernel encerrou {n}"),
        None => "Faltou memória na VM e o kernel encerrou um processo".to_string(),
    }
}

#[cfg(test)]
mod testes {
    use super::traduzir_oom;

    #[test]
    fn nomeia_o_processo_encerrado() {
        assert_eq!(
            traduzir_oom("Out of memory: Killed process 4821 (node) total-vm:4120332kB"),
            "Faltou memória na VM e o kernel encerrou node"
        );
    }

    #[test]
    fn sem_nome_fica_generico() {
        assert_eq!(traduzir_oom("Killed process 4821"), "Faltou memória na VM e o kernel encerrou um processo");
    }
}
