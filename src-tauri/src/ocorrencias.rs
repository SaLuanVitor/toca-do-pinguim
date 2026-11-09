//! Registro de ocorrências: cada trava do WSL, sobrecarga da VM, Docker sem resposta ou
//! devolução de cache que falhou vira um registro com o que veio antes (a carga dos últimos
//! 10 min e quem pesava), os picos enquanto durou e como terminou. É o que permite comparar
//! uma trava com a outra e medir se um tratamento ajudou.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;

use log::warn;
use serde::{Deserialize, Serialize};

use crate::pressao::Pesado;

/// Uma leitura a cada 30 s; 20 leituras são os 10 min antes da ocorrência.
const LEITURAS: usize = 20;
/// Ocorrências guardadas no arquivo; as mais velhas saem.
const GUARDADAS: usize = 200;

/// A carga num instante, como a ocorrência a guarda.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Carga {
    pub quando_ms: u64,
    pub disco_full: f32,
    pub memoria_full: f32,
    pub cpu_vm: f32,
    #[serde(default)]
    pub pesados: Vec<Pesado>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Ocorrencia {
    /// `trava`, `sobrecarga`, `docker` ou `devolucao`.
    pub tipo: String,
    pub titulo: String,
    pub inicio_ms: u64,
    /// Vazio enquanto dura; nas pontuais, igual ao início.
    pub fim_ms: Option<u64>,
    /// Como terminou: voltou sozinha, desligamento forçado, serviço reiniciado.
    pub saida: Option<String>,
    pub pico_disco: f32,
    pub pico_cpu: f32,
    /// A carga nos 10 min antes, da mais velha para a mais nova.
    pub antes: Vec<Carga>,
}

#[derive(Default)]
struct Estado {
    caminho: Option<PathBuf>,
    lista: Vec<Ocorrencia>,
    cargas: VecDeque<Carga>,
    /// Como a próxima trava que terminar saiu, anotado por quem a destravou.
    saida_da_trava: Option<String>,
}

static ESTADO: Mutex<Option<Estado>> = Mutex::new(None);

fn com<T>(f: impl FnOnce(&mut Estado) -> T) -> T {
    let mut e = ESTADO.lock().unwrap();
    f(e.get_or_insert_with(Estado::default))
}

/// Abre o arquivo da pasta de dados e carrega o que já foi registrado.
pub fn iniciar(dados: &std::path::Path) {
    let caminho = dados.join("ocorrencias.json");
    let lista = std::fs::read_to_string(&caminho).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    com(|e| {
        e.caminho = Some(caminho);
        e.lista = lista;
    });
}

fn gravar(e: &Estado) {
    let Some(caminho) = &e.caminho else { return };
    let texto = serde_json::to_string(&e.lista).unwrap_or_default();
    if let Err(erro) = std::fs::write(caminho, texto) {
        warn!("não foi possível gravar as ocorrências: {erro}");
    }
}

/// Guarda a carga do momento e atualiza os picos das ocorrências abertas.
pub fn observar(carga: Carga) {
    com(|e| {
        for o in e.lista.iter_mut().filter(|o| o.fim_ms.is_none()) {
            o.pico_disco = o.pico_disco.max(carga.disco_full);
            o.pico_cpu = o.pico_cpu.max(carga.cpu_vm);
        }
        e.cargas.push_back(carga);
        while e.cargas.len() > LEITURAS {
            e.cargas.pop_front();
        }
    });
}

/// Abre uma ocorrência do tipo, se não houver uma aberta, com a carga de antes.
pub fn abrir(tipo: &str, titulo: String, agora_ms: u64) {
    com(|e| {
        if e.lista.iter().any(|o| o.tipo == tipo && o.fim_ms.is_none()) {
            return;
        }
        let antes: Vec<Carga> = e.cargas.iter().cloned().collect();
        let pico_disco = antes.iter().map(|c| c.disco_full).fold(0.0, f32::max);
        let pico_cpu = antes.iter().map(|c| c.cpu_vm).fold(0.0, f32::max);
        e.lista.push(Ocorrencia { tipo: tipo.to_string(), titulo, inicio_ms: agora_ms, fim_ms: None, saida: None, pico_disco, pico_cpu, antes });
        let excesso = e.lista.len().saturating_sub(GUARDADAS);
        e.lista.drain(..excesso);
        gravar(e);
    });
}

/// Fecha a ocorrência aberta do tipo, se houver. Na trava, vale a saída anotada por quem a
/// destravou, quando houver.
pub fn fechar(tipo: &str, saida: &str, agora_ms: u64) {
    com(|e| {
        let anotada = if tipo == "trava" { e.saida_da_trava.take() } else { None };
        let Some(o) = e.lista.iter_mut().rev().find(|o| o.tipo == tipo && o.fim_ms.is_none()) else { return };
        o.fim_ms = Some(agora_ms);
        o.saida = Some(anotada.unwrap_or_else(|| saida.to_string()));
        gravar(e);
    });
}

/// Registra algo que acontece de uma vez, como uma devolução de cache que falhou.
pub fn pontual(tipo: &str, titulo: String, agora_ms: u64) {
    abrir(tipo, titulo, agora_ms);
    fechar(tipo, "registrada", agora_ms);
}

/// Quem destrava anota como, para a trava fechar com a saída certa. Sem trava aberta, a nota
/// não vale: ela iria parar na próxima trava.
pub fn anotar_saida_da_trava(saida: &str) {
    com(|e| {
        if e.lista.iter().any(|o| o.tipo == "trava" && o.fim_ms.is_none()) {
            e.saida_da_trava = Some(saida.to_string());
        }
    });
}

/// Da mais nova para a mais velha.
pub fn listar() -> Vec<Ocorrencia> {
    com(|e| e.lista.iter().rev().cloned().collect())
}

#[cfg(test)]
pub fn limpar_para_teste() {
    com(|e| *e = Estado::default());
}

#[cfg(test)]
mod testes {
    use super::*;

    // O estado é global: os testes que o usam não podem rodar em paralelo.
    static VEZ: Mutex<()> = Mutex::new(());

    fn carga(ms: u64, disco: f32, cpu: f32) -> Carga {
        Carga { quando_ms: ms, disco_full: disco, cpu_vm: cpu, ..Default::default() }
    }

    #[test]
    fn guarda_o_antes_os_picos_e_a_saida_anotada() {
        let _vez = VEZ.lock().unwrap();
        limpar_para_teste();
        for i in 0..25 {
            observar(carga(i * 30_000, i as f32, 10.0));
        }
        abrir("trava", "O WSL deixou de responder".into(), 1_000_000);
        abrir("trava", "repetida não abre outra".into(), 1_000_100);
        observar(carga(1_030_000, 40.0, 95.0));
        anotar_saida_da_trava("desligamento forçado pelo app");
        fechar("trava", "voltou sozinho", 1_060_000);
        let l = listar();
        assert_eq!(l.len(), 1);
        let o = &l[0];
        assert_eq!(o.antes.len(), 20);
        assert_eq!(o.antes[0].disco_full, 5.0);
        assert_eq!((o.pico_disco, o.pico_cpu), (40.0, 95.0));
        assert_eq!(o.saida.as_deref(), Some("desligamento forçado pelo app"));
        assert_eq!(o.fim_ms, Some(1_060_000));
    }

    #[test]
    fn pontual_ja_nasce_fechada_e_sem_anotacao_vale_a_saida_dada() {
        let _vez = VEZ.lock().unwrap();
        limpar_para_teste();
        pontual("devolucao", "Devolver o cache não terminou".into(), 5);
        // Desligamento sem trava aberta: a nota não pode ir parar na próxima trava.
        anotar_saida_da_trava("desligamento forçado pelo app");
        abrir("trava", "x".into(), 10);
        fechar("trava", "voltou sozinho", 20);
        let l = listar();
        assert_eq!(l[0].saida.as_deref(), Some("voltou sozinho"));
        assert_eq!((l[1].inicio_ms, l[1].fim_ms), (5, Some(5)));
    }
}
