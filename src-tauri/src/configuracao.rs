//! Comandos de configuração: o `.wslconfig` global e o `/etc/wsl.conf` de cada distro.
//! Os dois são INI e passam pelo mesmo editor, que troca só as linhas alteradas.

use std::io::Write;
use std::process::Stdio;
use std::sync::Arc;
use std::thread;

use log::info;
use serde::{Deserialize, Serialize};

use crate::config::{self, WslConfig};
use crate::{Estado, bloqueante, wsl};

#[derive(Serialize)]
pub struct ConteudoConfig {
    /// Onde o arquivo mora, para mostrar na tela.
    caminho: String,
    existe: bool,
    /// (seção, chave, valor), na ordem do arquivo.
    valores: Vec<(String, String, String)>,
}

#[derive(Deserialize)]
pub struct MudancaConfig {
    secao: String,
    chave: String,
    /// `None` tira a chave, e vale o padrão do WSL.
    valor: Option<String>,
}

/// Seções documentadas de cada arquivo. Gravar fora delas não teria efeito e poderia
/// esconder um erro de digitação.
const SECOES_WSLCONFIG: [&str; 3] = ["wsl2", "experimental", "general"];
const SECOES_WSLCONF: [&str; 7] = ["automount", "network", "interop", "user", "boot", "gpu", "time"];

/// Abaixo disto a VM mal sobe com systemd e Docker; é o piso que o app aceita para `memory`.
const MEMORIA_MINIMA: u64 = 1 << 30;

/// O que a máquina comporta, para a tela limitar os campos e o backend recusar o que passa.
#[derive(Serialize, Clone, Debug)]
pub struct Limites {
    pub ram_bytes: u64,
    pub processadores: u32,
    /// Espaço livre de cada disco fixo, para a swap não pedir mais do que cabe.
    pub discos: Vec<Disco>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Disco {
    pub letra: String,
    pub livre_bytes: u64,
}

pub fn limites() -> Limites {
    Limites {
        ram_bytes: crate::ram_host(),
        processadores: std::thread::available_parallelism().map_or(1, |n| n.get() as u32),
        discos: discos_fixos(),
    }
}

fn discos_fixos() -> Vec<Disco> {
    use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives};
    const DRIVE_FIXED: u32 = 3;
    // SAFETY: as chamadas recebem uma raiz como `C:\` terminada em zero e ponteiros válidos.
    let mascara = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mascara & (1 << i) != 0)
        .filter_map(|i| {
            let letra = (b'A' + i) as char;
            let raiz: Vec<u16> = format!("{letra}:\\").encode_utf16().chain([0]).collect();
            if unsafe { GetDriveTypeW(raiz.as_ptr()) } != DRIVE_FIXED {
                return None;
            }
            let mut livre = 0u64;
            let ok = unsafe { GetDiskFreeSpaceExW(raiz.as_ptr(), &mut livre, std::ptr::null_mut(), std::ptr::null_mut()) } != 0;
            ok.then(|| Disco { letra: letra.to_string(), livre_bytes: livre })
        })
        .collect()
}

#[tauri::command]
pub fn limites_da_maquina() -> Limites {
    limites()
}

/// O que a tela já limita, conferido de novo aqui: o arquivo também pode chegar por quem
/// chama o comando direto.
fn conferir_limites(m: &MudancaConfig, l: &Limites) -> Result<(), String> {
    let Some(valor) = m.valor.as_deref() else { return Ok(()) };
    let gb = |b: u64| format!("{:.1}", b as f64 / (1u64 << 30) as f64).replace('.', ",");
    match (m.secao.to_ascii_lowercase().as_str(), m.chave.to_ascii_lowercase().as_str()) {
        ("wsl2", "memory") => {
            let b = config::tamanho_em_bytes(valor).ok_or("memory: use um tamanho como 8GB")?;
            if b < MEMORIA_MINIMA || b > l.ram_bytes {
                return Err(format!("memory precisa ficar entre 1 GB e {} GB, a RAM do Windows", gb(l.ram_bytes)));
            }
        }
        ("wsl2", "swap") => {
            config::tamanho_em_bytes(valor).ok_or("swap: use um tamanho como 4GB, ou 0")?;
        }
        ("wsl2", "processors") => {
            let n: u32 = valor.trim().parse().map_err(|_| "processors: use um número inteiro")?;
            if n < 1 || n > l.processadores {
                return Err(format!("processors precisa ficar entre 1 e {}, os processadores lógicos da máquina", l.processadores));
            }
        }
        _ => {}
    }
    Ok(())
}

fn validar(m: &MudancaConfig, secoes: &[&str]) -> Result<(), String> {
    if !secoes.iter().any(|s| s.eq_ignore_ascii_case(&m.secao)) {
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

/// Aplica as mudanças e devolve o resumo "chave: antes para depois" de cada uma.
fn aplicar(cfg: &mut WslConfig, mudancas: &[MudancaConfig]) -> String {
    let mut resumo = Vec::new();
    for m in mudancas {
        let antes = cfg.obter(&m.secao, &m.chave).unwrap_or_else(|| "padrão".into());
        match &m.valor {
            Some(v) => cfg.definir(&m.secao, &m.chave, v),
            None => cfg.remover(&m.secao, &m.chave),
        }
        resumo.push(format!("{}: {antes} para {}", m.chave, m.valor.as_deref().unwrap_or("padrão")));
    }
    resumo.join("; ")
}

#[tauri::command]
pub fn ler_wslconfig() -> ConteudoConfig {
    let caminho = config::caminho();
    ConteudoConfig {
        existe: caminho.is_file(),
        caminho: caminho.display().to_string(),
        valores: WslConfig::ler().todas(),
    }
}

/// O `.wslconfig` só vale quando a VM reinicia; com `reiniciar`, o app desliga o WSL.
#[tauri::command]
pub async fn gravar_wslconfig(
    app: tauri::AppHandle,
    estado: Estado<'_>,
    mudancas: Vec<MudancaConfig>,
    reiniciar: bool,
) -> Result<(), String> {
    mudancas.iter().try_for_each(|m| validar(m, &SECOES_WSLCONFIG))?;
    let maquina = limites();
    mudancas.iter().try_for_each(|m| conferir_limites(m, &maquina))?;
    if mudancas.is_empty() {
        return Ok(());
    }
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        let mut cfg = WslConfig::ler();
        let resumo = aplicar(&mut cfg, &mudancas);
        cfg.gravar().map_err(|e| format!("não foi possível gravar o .wslconfig: {e}"))?;
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

const WSL_CONF: &str = "/etc/wsl.conf";

/// Lê o `/etc/wsl.conf` de dentro da distro. Ler liga a distro, se estiver desligada: a
/// tela só chama isto com a distro ligada ou a pedido explícito.
fn ler_texto_wslconf(distro: &str) -> Result<Option<String>, String> {
    // A ausência sai como código 3, e não pelo texto de erro do `cat`, que vem traduzido
    // conforme o idioma da distro.
    let script = r#"if [ -e /etc/wsl.conf ]; then cat /etc/wsl.conf; else exit 3; fi"#;
    let saida = wsl::com_prazo(wsl::wsl().args(["-d", distro, "-u", "root", "--exec", "sh", "-c", script]), wsl::PRAZO)?;
    match saida.status.code() {
        Some(0) => Ok(Some(wsl::decodificar(&saida.stdout))),
        Some(3) => Ok(None),
        _ => Err(format!(
            "{distro}: não foi possível ler {WSL_CONF}: {}",
            wsl::decodificar(&saida.stderr).trim()
        )),
    }
}

#[tauri::command]
pub async fn ler_wslconf(distro: String) -> Result<ConteudoConfig, String> {
    bloqueante(move || {
        let texto = ler_texto_wslconf(&distro)?;
        Ok(ConteudoConfig {
            caminho: format!("{distro}:{WSL_CONF}"),
            existe: texto.is_some(),
            valores: WslConfig::de_texto(texto.as_deref().unwrap_or_default()).todas(),
        })
    })
    .await
}

/// Grava pelo stdin do `wsl.exe`, como a cópia do agente: a versão anterior fica em
/// `/etc/wsl.conf.toca-do-pinguim.bak`, e o arquivo novo entra por troca atômica.
fn gravar_texto_wslconf(distro: &str, texto: String) -> Result<(), String> {
    let script = r#"set -e; f=/etc/wsl.conf; [ -f "$f" ] && cp -p "$f" "$f.toca-do-pinguim.bak"; cat > "$f.tmp"; chmod 644 "$f.tmp"; mv -f "$f.tmp" "$f""#;
    let mut filho = wsl::wsl()
        .args(["-d", distro, "-u", "root", "--exec", "sh", "-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("wsl.exe: {e}"))?;
    let mut entrada = filho.stdin.take().expect("stdin pedido como piped");
    thread::spawn(move || {
        let _ = entrada.write_all(texto.as_bytes());
    });
    let saida = wsl::esperar(filho, wsl::PRAZO, &format!("gravar {WSL_CONF} em {distro}"), Some(distro))?;
    if saida.status.success() {
        Ok(())
    } else {
        Err(format!("{distro}: não foi possível gravar {WSL_CONF}: {}", wsl::decodificar(&saida.stderr).trim()))
    }
}

/// O `wsl.conf` vale quando a distro reinicia; com `reiniciar`, o app termina só ela.
#[tauri::command]
pub async fn gravar_wslconf(
    app: tauri::AppHandle,
    estado: Estado<'_>,
    distro: String,
    mudancas: Vec<MudancaConfig>,
    reiniciar: bool,
) -> Result<(), String> {
    mudancas.iter().try_for_each(|m| validar(m, &SECOES_WSLCONF))?;
    if mudancas.is_empty() {
        return Ok(());
    }
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        let atual = ler_texto_wslconf(&distro)?;
        let mut cfg = WslConfig::de_texto(atual.as_deref().unwrap_or_default());
        let resumo = aplicar(&mut cfg, &mudancas);
        gravar_texto_wslconf(&distro, cfg.texto())?;
        info!("[{distro}] wsl.conf alterado (reiniciar: {reiniciar}): {resumo}");
        agentes.registrar(&app, "config", Some(&distro), format!("wsl.conf de {distro}: {resumo}"));
        if reiniciar {
            agentes.parar(&distro);
            wsl::terminar(&distro)?;
            agentes.registrar(&app, "distro", Some(&distro), format!("O app terminou {distro} para aplicar o wsl.conf"));
        }
        Ok(())
    })
    .await
}

#[cfg(test)]
mod testes {
    use super::*;

    fn m(secao: &str, chave: &str, valor: Option<&str>) -> MudancaConfig {
        MudancaConfig { secao: secao.into(), chave: chave.into(), valor: valor.map(str::to_string) }
    }

    #[test]
    fn recusa_memoria_e_processadores_fora_da_maquina() {
        let l = Limites { ram_bytes: 32 << 30, processadores: 8, discos: Vec::new() };
        let m = |chave: &str, v: &str| MudancaConfig { secao: "wsl2".into(), chave: chave.into(), valor: Some(v.into()) };
        assert!(conferir_limites(&m("memory", "12GB"), &l).is_ok());
        assert!(conferir_limites(&m("memory", "32GB"), &l).is_ok());
        assert!(conferir_limites(&m("memory", "33GB"), &l).is_err());
        assert!(conferir_limites(&m("memory", "512MB"), &l).is_err());
        assert!(conferir_limites(&m("memory", "doze"), &l).is_err());
        assert!(conferir_limites(&m("processors", "8"), &l).is_ok());
        assert!(conferir_limites(&m("processors", "9"), &l).is_err());
        assert!(conferir_limites(&m("processors", "0"), &l).is_err());
        assert!(conferir_limites(&m("swap", "0"), &l).is_ok());
        assert!(conferir_limites(&m("swap", "x"), &l).is_err());
        // Voltar ao padrão sempre pode.
        let padrao = MudancaConfig { secao: "wsl2".into(), chave: "memory".into(), valor: None };
        assert!(conferir_limites(&padrao, &l).is_ok());
    }

    #[test]
    fn a_maquina_tem_ram_processador_e_o_disco_do_sistema() {
        let l = limites();
        assert!(l.ram_bytes > 0 && l.processadores >= 1);
        assert!(l.discos.iter().any(|d| d.letra == "C" && d.livre_bytes > 0));
    }
    #[test]
    fn recusa_secao_de_outro_arquivo_chave_estranha_e_quebra_de_linha() {
        assert!(validar(&m("boot", "systemd", Some("true")), &SECOES_WSLCONF).is_ok());
        assert!(validar(&m("wsl2", "memory", Some("8GB")), &SECOES_WSLCONF).is_err());
        assert!(validar(&m("boot", "sys temd", Some("true")), &SECOES_WSLCONF).is_err());
        assert!(validar(&m("boot", "command", Some("a\n[user]\ndefault=root")), &SECOES_WSLCONF).is_err());
    }

    #[test]
    fn aplica_e_resume_as_mudancas() {
        let mut cfg = WslConfig::de_texto("[boot]\nsystemd=true\n\n[user]\ndefault=luan\n");
        let resumo = aplicar(&mut cfg, &[m("boot", "systemd", None), m("network", "hostname", Some("toca"))]);
        assert_eq!(resumo, "systemd: true para padrão; hostname: padrão para toca");
        assert_eq!(cfg.texto(), "[boot]\n\n[user]\ndefault=luan\n\n[network]\nhostname=toca\n");
    }
}
