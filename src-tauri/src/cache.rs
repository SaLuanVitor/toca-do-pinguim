//! Devolver ao Windows o cache da VM.
//!
//! O Linux guarda na memória o que leu do disco, e a VM só devolve ao Windows a memória
//! livre: cache não é livre, é descartável. Dentro da VM quase nunca falta memória, então o
//! kernel não descarta nada sozinho, e o `autoMemoryReclaim` do WSL só age com a VM ociosa.
//!
//! Medido numa máquina com um job de CI e 15 containers de pé: esvaziar devolveu 2,9 GB em
//! 30 s, e 5 min depois a VM estava 1,3 GB menor que antes. As tabelas de arquivos e os
//! blocos de disco voltaram em 2 min, porque algo continuava percorrendo arquivos. Por isso
//! o resultado é medido e mostrado, em vez de prometido.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use log::{info, warn};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::agentes::Agentes;
use crate::recursos::{self, gb, Recursos};
use crate::wsl;

const LOGO: Duration = Duration::from_secs(15);
/// O `sync` espera o disco gravar o que está pendente; com o disco ocupado, passou dos 30 s.
const PRAZO_ESVAZIAR: Duration = Duration::from_secs(5 * 60);
const DEPOIS: Duration = Duration::from_secs(120);
/// Intervalo mínimo entre duas devoluções automáticas: o cache volta em minutos, e esvaziar
/// sem parar só troca memória por leitura de disco.
const INTERVALO_AUTOMATICO: Duration = Duration::from_secs(10 * 60);

/// O `compact_memory` junta as páginas livres em blocos maiores, que o WSL devolve ao Windows
/// mais fácil; sem ele a devolução ainda acontece, então a falha dele não conta.
const ESVAZIAR: &str = "sync; echo 3 > /proc/sys/vm/drop_caches && { echo 1 > /proc/sys/vm/compact_memory 2>/dev/null; true; }";

#[derive(Serialize, Clone, Debug, Default)]
pub struct Devolucao {
    pub automatica: bool,
    pub quando_ms: u64,
    /// Memória da VM na RAM do Windows antes, 15 s depois e 2 min depois.
    pub antes_bytes: u64,
    pub logo_bytes: Option<u64>,
    pub depois_bytes: Option<u64>,
    pub concluida: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Preferencias {
    #[serde(default)]
    pub devolver_cache_automatico: bool,
    #[serde(default)]
    pub gatilho: Gatilho,
    #[serde(default)]
    pub limpeza: LimpezaProgramada,
    /// Fechar a janela deixa o app na área de notificação, em vez de sair.
    #[serde(default = "sim")]
    pub fechar_para_bandeja: bool,
    /// A notificação que explica onde o app ficou sai uma vez só.
    #[serde(default)]
    pub avisou_bandeja: bool,
}

fn sim() -> bool {
    true
}

impl Default for Preferencias {
    fn default() -> Self {
        Preferencias {
            devolver_cache_automatico: false,
            gatilho: Gatilho::default(),
            limpeza: LimpezaProgramada::default(),
            fechar_para_bandeja: true,
            avisou_bandeja: false,
        }
    }
}

/// Quando a devolução automática dispara. Basta um dos dois.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Gatilho {
    /// Memória disponível do Windows abaixo desta porcentagem da RAM.
    #[serde(default = "minimo_livre_padrao")]
    pub minimo_livre_pct: u8,
    /// Cache da VM acima deste tamanho; `None` não olha o cache.
    #[serde(default)]
    pub cache_maximo_bytes: Option<u64>,
}

/// O mesmo limiar do aviso de memória, que era o único gatilho antes de ser configurável.
fn minimo_livre_padrao() -> u8 {
    10
}

impl Default for Gatilho {
    fn default() -> Self {
        Gatilho { minimo_livre_pct: minimo_livre_padrao(), cache_maximo_bytes: None }
    }
}

/// Limpeza semanal, toda desligada até a pessoa escolher.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct LimpezaProgramada {
    #[serde(default)]
    pub cache_de_build: bool,
    #[serde(default)]
    pub sobras_de_containers: bool,
    /// Apagar do `/tmp` o que não muda há este tanto de dias; `None` não mexe no `/tmp`.
    #[serde(default)]
    pub tmp_dias: Option<u32>,
    #[serde(default)]
    pub ultima_ms: Option<u64>,
}

impl LimpezaProgramada {
    pub fn ligada(&self) -> bool {
        self.cache_de_build || self.sobras_de_containers || self.tmp_dias.is_some()
    }
}

impl Preferencias {
    pub fn caminho(dados: &Path) -> PathBuf {
        dados.join("preferencias.json")
    }

    /// Arquivo ausente ou ilegível volta ao padrão: tudo desligado.
    pub fn ler(caminho: &Path) -> Self {
        fs::read_to_string(caminho).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn gravar(&self, caminho: &Path) -> Result<(), String> {
        let texto = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(caminho, texto).map_err(|e| format!("não foi possível gravar as preferências: {e}"))
    }
}

/// `livre` é a fração da RAM do Windows disponível; `cache`, o cache descartável da VM.
pub fn pode_devolver_sozinho(ligado: bool, livre: f64, cache: Option<u64>, g: &Gatilho, desde_a_ultima: Option<Duration>) -> bool {
    let pouca_memoria = livre * 100.0 < f64::from(g.minimo_livre_pct);
    let muito_cache = matches!((cache, g.cache_maximo_bytes), (Some(c), Some(maximo)) if c > maximo);
    ligado && (pouca_memoria || muito_cache) && desde_a_ultima.is_none_or(|d| d >= INTERVALO_AUTOMATICO)
}

fn agora_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Esvazia o cache por uma distro já ligada, sem ligar nenhuma, e mede o efeito numa thread.
pub fn devolver(app: AppHandle, agentes: Arc<Agentes>, recursos: Arc<Recursos>, automatica: bool) -> Result<(), String> {
    if recursos.devolvendo.swap(true, Ordering::Relaxed) {
        return Err("o cache já está sendo devolvido; o resultado sai em 2 minutos".to_string());
    }
    // O intervalo conta da tentativa, não do sucesso: medido, uma falha às 01:23:24 virou outra
    // tentativa 5 s depois, e uma falha que se repetisse viraria uma tentativa a cada 5 s.
    let inicio = Instant::now();
    *recursos.ultima_devolucao.lock().unwrap() = Some(inicio);
    let resultado = esvaziar(&agentes, automatica);
    match resultado {
        Ok(antes) => {
            let mut d = Devolucao { automatica, quando_ms: agora_ms(), antes_bytes: antes, ..Default::default() };
            publicar(&app, &recursos, &d);
            thread::spawn(move || {
                thread::sleep(LOGO);
                d.logo_bytes = recursos::memoria_da_vm();
                publicar(&app, &recursos, &d);
                thread::sleep(DEPOIS.saturating_sub(inicio.elapsed()));
                d.depois_bytes = recursos::memoria_da_vm();
                d.concluida = true;
                let texto = descrever(&d);
                info!("{texto}");
                agentes.registrar(&app, "memoria", None, texto);
                publicar(&app, &recursos, &d);
                recursos.devolvendo.store(false, Ordering::Relaxed);
            });
            Ok(())
        }
        Err(e) => {
            recursos.devolvendo.store(false, Ordering::Relaxed);
            warn!("devolver o cache falhou: {e}");
            Err(e)
        }
    }
}

fn esvaziar(agentes: &Agentes, automatica: bool) -> Result<u64, String> {
    let distro = agentes.uma_ligada().ok_or("nenhuma distro está ligada; sem VM, não há cache para devolver")?;
    let antes = recursos::memoria_da_vm().ok_or("o WSL não está rodando")?;
    info!("devolvendo o cache da VM por {distro} ({})", if automatica { "automático" } else { "a pedido" });
    let saida = wsl::executar_longo(&["-d", &distro, "-u", "root", "--exec", "sh", "-c", ESVAZIAR], PRAZO_ESVAZIAR)?;
    if !saida.status.success() {
        return Err(format!("{distro} recusou: {}", wsl::decodificar(&saida.stderr).trim()));
    }
    Ok(antes)
}

fn publicar(app: &AppHandle, recursos: &Recursos, d: &Devolucao) {
    *recursos.devolucao.lock().unwrap() = Some(d.clone());
    let _ = app.emit("devolucao", d);
}

pub fn descrever(d: &Devolucao) -> String {
    let quem = if d.automatica { "O app devolveu sozinho" } else { "O app devolveu" };
    match (d.logo_bytes, d.depois_bytes) {
        (Some(logo), Some(depois)) => {
            let ficou = d.antes_bytes.saturating_sub(depois);
            format!(
                "{quem} o cache da VM: ela foi de {} GB para {} GB na RAM do Windows em 15 s e estava em {} GB 2 min depois, {} GB a menos que antes",
                gb(d.antes_bytes),
                gb(logo),
                gb(depois),
                gb(ficou)
            )
        }
        _ => format!("{quem} o cache da VM, que estava com {} GB na RAM do Windows", gb(d.antes_bytes)),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const GIB: u64 = 1 << 30;

    #[test]
    fn sozinho_so_com_pressao_vm_ligada_e_intervalo_vencido() {
        let g = Gatilho::default();
        assert!(pode_devolver_sozinho(true, 0.09, None, &g, None));
        assert!(!pode_devolver_sozinho(false, 0.09, None, &g, None));
        assert!(!pode_devolver_sozinho(true, 0.11, None, &g, None));
        assert!(!pode_devolver_sozinho(true, 0.09, None, &g, Some(Duration::from_secs(9 * 60))));
        assert!(pode_devolver_sozinho(true, 0.09, None, &g, Some(INTERVALO_AUTOMATICO)));
    }

    #[test]
    fn gatilho_por_cache_vale_mesmo_com_memoria_sobrando() {
        let g = Gatilho { minimo_livre_pct: 10, cache_maximo_bytes: Some(4 * GIB) };
        assert!(pode_devolver_sozinho(true, 0.40, Some(5 * GIB), &g, None));
        assert!(!pode_devolver_sozinho(true, 0.40, Some(3 * GIB), &g, None));
        // Sem leitura do cache, só o gatilho de memória conta.
        assert!(!pode_devolver_sozinho(true, 0.40, None, &g, None));
        let mais = Gatilho { minimo_livre_pct: 20, cache_maximo_bytes: None };
        assert!(pode_devolver_sozinho(true, 0.15, None, &mais, None));
    }

    #[test]
    fn descreve_o_que_ficou_depois_de_dois_minutos() {
        let d = Devolucao {
            automatica: true,
            antes_bytes: 11 * GIB + GIB / 2,
            logo_bytes: Some(8 * GIB + GIB / 2),
            depois_bytes: Some(10 * GIB + GIB / 4),
            concluida: true,
            ..Default::default()
        };
        let texto = descrever(&d);
        assert!(texto.starts_with("O app devolveu sozinho"));
        assert!(texto.contains("de 11,5 GB para 8,5 GB"));
        assert!(texto.contains("1,2 GB a menos"));
    }

    #[test]
    fn preferencias_antigas_ganham_o_gatilho_padrao() {
        let antiga: Preferencias = serde_json::from_str(r#"{ "devolver_cache_automatico": true }"#).unwrap();
        assert!(antiga.devolver_cache_automatico);
        assert_eq!(antiga.gatilho, Gatilho::default());
        assert_eq!(antiga.gatilho.minimo_livre_pct, 10);
    }

    #[test]
    fn preferencias_ausentes_ou_quebradas_voltam_ao_padrao() {
        let pasta = std::env::temp_dir().join(format!("toca-prefs-{}", std::process::id()));
        fs::create_dir_all(&pasta).unwrap();
        let caminho = Preferencias::caminho(&pasta);
        assert_eq!(Preferencias::ler(&caminho), Preferencias::default());
        fs::write(&caminho, "{ quebrado").unwrap();
        assert_eq!(Preferencias::ler(&caminho), Preferencias::default());
        let ligada = Preferencias { devolver_cache_automatico: true, ..Default::default() };
        ligada.gravar(&caminho).unwrap();
        assert_eq!(Preferencias::ler(&caminho), ligada);
        fs::remove_dir_all(&pasta).unwrap();
    }
}
