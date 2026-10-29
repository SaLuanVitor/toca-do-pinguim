//! Chamadas ao `wsl.exe`.
//!
//! Toda chamada sai com `CREATE_NO_WINDOW`: o app não tem console, e sem a flag cada
//! `wsl.exe` piscaria uma janela preta na tela.

use std::os::windows::process::CommandExt;
use std::process::Command;

use serde::Serialize;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn wsl() -> Command {
    let mut c = Command::new("wsl.exe");
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// Parte dos verbos do `wsl.exe` responde em UTF-16, mesmo com o console em UTF-8.
pub fn decodificar(bytes: &[u8]) -> String {
    let texto = if bytes.contains(&0) {
        let u: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    texto.replace('\u{feff}', "")
}

fn executar(args: &[&str]) -> Result<String, String> {
    let saida = wsl().args(args).output().map_err(|e| format!("wsl.exe: {e}"))?;
    let stdout = decodificar(&saida.stdout);
    if saida.status.success() {
        Ok(stdout)
    } else {
        let stderr = decodificar(&saida.stderr);
        Err(format!("{} {}", stdout.trim(), stderr.trim()).trim().to_string())
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Distro {
    pub nome: String,
    pub versao: u8,
    pub padrao: bool,
    pub rodando: bool,
}

/// O estado vem de `--running` e não da coluna STATE do `--verbose`, que pode vir
/// traduzida conforme o idioma do Windows.
pub fn listar() -> Result<Vec<Distro>, String> {
    let verbose = executar(&["--list", "--verbose"])?;
    let rodando = rodando();
    Ok(verbose
        .lines()
        .skip(1)
        .filter_map(|linha| {
            let linha = linha.trim();
            let padrao = linha.starts_with('*');
            let mut campos = linha.trim_start_matches('*').split_whitespace();
            let nome = campos.next()?.to_string();
            let versao = campos.last()?.parse().ok()?;
            Some(Distro {
                rodando: rodando.contains(&nome),
                nome,
                versao,
                padrao,
            })
        })
        .collect())
}

/// Sem distro rodando o `wsl.exe` sai com erro; para quem pergunta, isso é lista vazia.
pub fn rodando() -> Vec<String> {
    executar(&["--list", "--running", "--quiet"])
        .map(|s| {
            s.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub fn iniciar(nome: &str) -> Result<(), String> {
    executar(&["-d", nome, "--exec", "true"]).map(|_| ())
}

pub fn terminar(nome: &str) -> Result<(), String> {
    executar(&["--terminate", nome]).map(|_| ())
}

pub fn desligar() -> Result<(), String> {
    executar(&["--shutdown"]).map(|_| ())
}
