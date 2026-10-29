//! Pico de memória da VM por minuto, guardado em disco para sugerir o limite do WSL.
//!
//! "Usado" é `MemTotal - MemAvailable`: fica de fora o cache que o Linux devolve quando
//! falta memória, que é justamente o que faz o `vmmemWSL` parecer maior do que a
//! necessidade real.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use protocolo::Amostra;
use serde::Serialize;

const GIB: u64 = 1 << 30;
const MINUTOS_GUARDADOS: u64 = 30 * 24 * 60;
const JANELA_SUGESTAO: u64 = 7 * 24 * 60;
/// Abaixo de uma hora de dados o pico ainda não diz nada sobre o uso típico.
const MINUTOS_MINIMOS: usize = 60;
/// Política do app: folga de 25% sobre o pico e piso de 2 GB.
const FOLGA: f64 = 1.25;
const PISO: u64 = 2 * GIB;

#[derive(Clone, Copy)]
struct Ponto {
    minuto: u64,
    usado: u64,
}

pub struct Historico {
    arquivo: PathBuf,
    pontos: Vec<Ponto>,
    minuto_atual: u64,
    pico_atual: u64,
}

#[derive(Serialize)]
pub struct Resumo {
    pub minutos_de_dados: usize,
    pub pico_bytes: u64,
    pub sugestao_bytes: Option<u64>,
}

pub fn minuto_agora() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() / 60).unwrap_or(0)
}

impl Historico {
    pub fn abrir(arquivo: PathBuf) -> Self {
        let corte = minuto_agora().saturating_sub(MINUTOS_GUARDADOS);
        let pontos: Vec<Ponto> = fs::read_to_string(&arquivo)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let (m, u) = l.split_once(',')?;
                Some(Ponto { minuto: m.parse().ok()?, usado: u.parse().ok()? })
            })
            .filter(|p| p.minuto >= corte)
            .collect();
        // Regrava só o que está dentro da janela, para o arquivo não crescer sem fim.
        let texto: String = pontos.iter().map(|p| format!("{},{}\n", p.minuto, p.usado)).collect();
        let _ = fs::write(&arquivo, texto);
        Historico { arquivo, pontos, minuto_atual: 0, pico_atual: 0 }
    }

    /// Todas as distros veem a mesma VM; amostras de distros diferentes no mesmo minuto
    /// só disputam o pico.
    pub fn registrar(&mut self, a: &Amostra) {
        let usado = a.mem_total.saturating_sub(a.mem_disponivel);
        let minuto = minuto_agora();
        if minuto != self.minuto_atual {
            self.fechar_minuto();
            self.minuto_atual = minuto;
        }
        self.pico_atual = self.pico_atual.max(usado);
    }

    fn fechar_minuto(&mut self) {
        if self.minuto_atual == 0 || self.pico_atual == 0 {
            return;
        }
        let p = Ponto { minuto: self.minuto_atual, usado: self.pico_atual };
        self.pontos.push(p);
        self.pico_atual = 0;
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.arquivo) {
            let _ = writeln!(f, "{},{}", p.minuto, p.usado);
        }
    }

    pub fn resumo(&self, teto_bytes: u64) -> Resumo {
        let corte = minuto_agora().saturating_sub(JANELA_SUGESTAO);
        let janela: Vec<u64> = self
            .pontos
            .iter()
            .filter(|p| p.minuto >= corte)
            .map(|p| p.usado)
            .chain((self.pico_atual > 0).then_some(self.pico_atual))
            .collect();
        let pico = janela.iter().copied().max().unwrap_or(0);
        let sugestao = (janela.len() >= MINUTOS_MINIMOS).then(|| {
            let gib = ((pico as f64 * FOLGA) / GIB as f64).ceil() as u64 * GIB;
            gib.clamp(PISO, teto_bytes.max(PISO))
        });
        Resumo { minutos_de_dados: janela.len(), pico_bytes: pico, sugestao_bytes: sugestao }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn com_pontos(pontos: Vec<(u64, f64)>) -> Historico {
        let agora = minuto_agora();
        Historico {
            arquivo: PathBuf::new(),
            pontos: pontos
                .into_iter()
                .map(|(atras, gib)| Ponto { minuto: agora - atras, usado: (gib * GIB as f64) as u64 })
                .collect(),
            minuto_atual: 0,
            pico_atual: 0,
        }
    }

    fn uma_hora_com_pico(pico_gib: f64) -> Vec<(u64, f64)> {
        (1..=60).map(|m| (m, if m == 30 { pico_gib } else { 1.0 })).collect()
    }

    #[test]
    fn sem_uma_hora_nao_sugere() {
        let r = com_pontos((1..60).map(|m| (m, 3.0)).collect()).resumo(32 * GIB);
        assert_eq!(r.minutos_de_dados, 59);
        assert_eq!(r.pico_bytes, 3 * GIB);
        assert_eq!(r.sugestao_bytes, None);
    }

    #[test]
    fn sugere_pico_com_folga_arredondado_para_cima() {
        // 7 GiB * 1,25 = 8,75 GiB, que vira 9.
        let r = com_pontos(uma_hora_com_pico(7.0)).resumo(32 * GIB);
        assert_eq!(r.sugestao_bytes, Some(9 * GIB));
    }

    #[test]
    fn respeita_piso_e_teto() {
        assert_eq!(com_pontos(uma_hora_com_pico(0.5)).resumo(32 * GIB).sugestao_bytes, Some(2 * GIB));
        assert_eq!(com_pontos(uma_hora_com_pico(30.0)).resumo(16 * GIB).sugestao_bytes, Some(16 * GIB));
    }

    #[test]
    fn pico_de_mais_de_sete_dias_nao_entra() {
        let mut pontos = uma_hora_com_pico(4.0);
        pontos.push((8 * 24 * 60, 20.0));
        let r = com_pontos(pontos).resumo(32 * GIB);
        assert_eq!(r.pico_bytes, 4 * GIB);
        assert_eq!(r.sugestao_bytes, Some(5 * GIB));
    }
}
