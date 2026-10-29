//! Leitura e escrita do `%UserProfile%\.wslconfig` sem perder o que o app não conhece.
//!
//! O arquivo é INI. Ao gravar, só a linha da chave alterada muda: comentários, ordem,
//! seções e chaves desconhecidas ficam como estavam. Seção e chave são comparadas sem
//! diferenciar maiúsculas, porque a própria documentação do WSL escreve `swapFile` e
//! `swapfile` para a mesma chave.

use std::fs;
use std::io;
use std::path::PathBuf;

pub fn caminho() -> PathBuf {
    let perfil = std::env::var_os("USERPROFILE").unwrap_or_default();
    PathBuf::from(perfil).join(".wslconfig")
}

pub struct WslConfig {
    linhas: Vec<String>,
    crlf: bool,
}

impl WslConfig {
    pub fn ler() -> Self {
        let texto = fs::read_to_string(caminho()).unwrap_or_default();
        let texto = texto.trim_start_matches('\u{feff}');
        WslConfig {
            crlf: texto.contains("\r\n"),
            linhas: texto.lines().map(str::to_string).collect(),
        }
    }

    pub fn obter(&self, secao: &str, chave: &str) -> Option<String> {
        let mut atual = String::new();
        for linha in &self.linhas {
            if let Some(s) = cabecalho(linha) {
                atual = s;
            } else if atual.eq_ignore_ascii_case(secao) {
                if let Some((k, v)) = par(linha) {
                    if k.eq_ignore_ascii_case(chave) {
                        return Some(v);
                    }
                }
            }
        }
        None
    }

    pub fn definir(&mut self, secao: &str, chave: &str, valor: &str) {
        let mut atual = String::new();
        let mut ultima_da_secao: Option<usize> = None;
        for i in 0..self.linhas.len() {
            if let Some(s) = cabecalho(&self.linhas[i]) {
                atual = s;
                if atual.eq_ignore_ascii_case(secao) {
                    ultima_da_secao = Some(i);
                }
                continue;
            }
            if !atual.eq_ignore_ascii_case(secao) {
                continue;
            }
            if let Some((k, _)) = par(&self.linhas[i]) {
                if k.eq_ignore_ascii_case(chave) {
                    // Mantém a grafia da chave que já estava no arquivo.
                    self.linhas[i] = format!("{k}={valor}");
                    return;
                }
            }
            if !self.linhas[i].trim().is_empty() {
                ultima_da_secao = Some(i);
            }
        }
        match ultima_da_secao {
            Some(i) => self.linhas.insert(i + 1, format!("{chave}={valor}")),
            None => {
                if self.linhas.last().is_some_and(|l| !l.trim().is_empty()) {
                    self.linhas.push(String::new());
                }
                self.linhas.push(format!("[{secao}]"));
                self.linhas.push(format!("{chave}={valor}"));
            }
        }
    }

    /// Tira a chave da seção, o que devolve o valor padrão do WSL. O cabeçalho da seção
    /// fica, mesmo vazio.
    pub fn remover(&mut self, secao: &str, chave: &str) {
        let mut atual = String::new();
        self.linhas.retain(|linha| {
            if let Some(s) = cabecalho(linha) {
                atual = s;
                return true;
            }
            !(atual.eq_ignore_ascii_case(secao) && par(linha).is_some_and(|(k, _)| k.eq_ignore_ascii_case(chave)))
        });
    }

    /// Todas as chaves do arquivo, na ordem em que aparecem, inclusive as que o app não
    /// conhece.
    pub fn todas(&self) -> Vec<(String, String, String)> {
        let mut atual = String::new();
        let mut saida = Vec::new();
        for linha in &self.linhas {
            if let Some(s) = cabecalho(linha) {
                atual = s;
            } else if let Some((k, v)) = par(linha) {
                saida.push((atual.clone(), k, v));
            }
        }
        saida
    }

    /// Grava por arquivo temporário e troca, guardando a versão anterior ao lado: um
    /// `.wslconfig` corrompido não impede o WSL de subir, mas faz ele ignorar o arquivo
    /// inteiro sem avisar.
    pub fn gravar(&self) -> io::Result<()> {
        let destino = caminho();
        if destino.exists() {
            fs::copy(&destino, destino.with_file_name(".wslconfig.toca-do-pinguim.bak"))?;
        }
        let quebra = if self.crlf { "\r\n" } else { "\n" };
        let mut texto = self.linhas.join(quebra);
        texto.push_str(quebra);
        let temporario = destino.with_file_name(".wslconfig.toca-do-pinguim.tmp");
        fs::write(&temporario, texto)?;
        fs::rename(&temporario, &destino)
    }

    /// `instanceIdleTimeout` em `[general]`; -1 desliga o desligamento por ociosidade.
    pub fn espera_ociosidade_ms(&self) -> i64 {
        self.obter("general", "instanceIdleTimeout")
            .and_then(|v| v.parse().ok())
            .unwrap_or(15_000)
    }
}

fn cabecalho(linha: &str) -> Option<String> {
    let t = linha.trim();
    (t.starts_with('[') && t.ends_with(']')).then(|| t[1..t.len() - 1].trim().to_string())
}

fn par(linha: &str) -> Option<(String, String)> {
    let t = linha.trim();
    if t.starts_with('#') || t.starts_with(';') {
        return None;
    }
    let (k, v) = t.split_once('=')?;
    let v = v.split(" #").next().unwrap_or(v).trim().trim_matches('"');
    Some((k.trim().to_string(), v.to_string()))
}

/// Tamanho no formato do `.wslconfig`: número com unidade opcional (`8GB`, `512MB`); sem
/// unidade, bytes.
pub fn tamanho_em_bytes(valor: &str) -> Option<u64> {
    let v = valor.trim().to_ascii_uppercase();
    let corte = v.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(v.len());
    let (numero, unidade) = v.split_at(corte);
    let numero: f64 = numero.parse().ok()?;
    let fator: u64 = match unidade.trim() {
        "" | "B" => 1,
        "K" | "KB" => 1 << 10,
        "M" | "MB" => 1 << 20,
        "G" | "GB" => 1 << 30,
        "T" | "TB" => 1 << 40,
        _ => return None,
    };
    Some((numero * fator as f64) as u64)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn de(texto: &str) -> WslConfig {
        WslConfig { linhas: texto.lines().map(str::to_string).collect(), crlf: false }
    }

    #[test]
    fn troca_so_a_linha_da_chave() {
        let mut c = de("# meu comentário\n[wsl2]\nMemory=12GB # antigo\nprocessors=8\n\n[experimental]\nsparseVhd=true");
        c.definir("WSL2", "memory", "10GB");
        assert_eq!(
            c.linhas.join("\n"),
            "# meu comentário\n[wsl2]\nMemory=10GB\nprocessors=8\n\n[experimental]\nsparseVhd=true"
        );
    }

    #[test]
    fn acrescenta_na_secao_existente_e_cria_a_que_falta() {
        let mut c = de("[wsl2]\nprocessors=8\n\n[experimental]\nsparseVhd=true");
        c.definir("wsl2", "memory", "6GB");
        c.definir("general", "instanceIdleTimeout", "30000");
        assert_eq!(
            c.linhas.join("\n"),
            "[wsl2]\nprocessors=8\nmemory=6GB\n\n[experimental]\nsparseVhd=true\n\n[general]\ninstanceIdleTimeout=30000"
        );
        assert_eq!(c.espera_ociosidade_ms(), 30000);
    }

    #[test]
    fn remove_so_a_chave_da_secao_certa() {
        let mut c = de("[wsl2]\nswap=0\nmemory=8GB\n\n[experimental]\nswap=1");
        c.remover("WSL2", "SWAP");
        assert_eq!(c.linhas.join("\n"), "[wsl2]\nmemory=8GB\n\n[experimental]\nswap=1");
        assert_eq!(
            c.todas(),
            vec![
                ("wsl2".into(), "memory".into(), "8GB".into()),
                ("experimental".into(), "swap".into(), "1".into())
            ]
        );
    }

    #[test]
    fn le_tamanhos() {
        assert_eq!(tamanho_em_bytes("12GB"), Some(12 << 30));
        assert_eq!(tamanho_em_bytes("512mb"), Some(512 << 20));
        assert_eq!(tamanho_em_bytes("1048576"), Some(1 << 20));
        assert_eq!(tamanho_em_bytes("doze"), None);
    }
}
