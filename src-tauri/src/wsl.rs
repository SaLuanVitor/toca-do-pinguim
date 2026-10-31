//! Chamadas ao `wsl.exe`.
//!
//! Toda chamada sai com `CREATE_NO_WINDOW`: o app não tem console, e sem a flag cada
//! `wsl.exe` piscaria uma janela preta na tela.

use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Output, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use log::{debug, error, info, warn};
use serde::Serialize;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub const PRAZO: Duration = Duration::from_secs(30);
/// Desligar a VM espera todas as distros terminarem. Com o WSL travado, medido, levou
/// 117 s.
const PRAZO_DESLIGAR: Duration = Duration::from_secs(180);
/// Acima disto a chamada vai ao log como lenta, mesmo dando certo.
const LENTA: Duration = Duration::from_secs(5);

pub fn wsl() -> Command {
    let mut c = Command::new("wsl.exe");
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// Um `wsl.exe` preso é uma sessão aberta, e sessão aberta segura a distro ligada: foi
/// assim que uma distro de teste ficou de pé 69 s em vez de 15. Toda chamada tem prazo e
/// é encerrada quando ele vence. A saída é lida só no fim, o que serve às respostas
/// curtas do `wsl.exe`; quem transmite muito (o agente) não passa por aqui.
///
/// `alvo` diz o que a chamada exercita, para o log e para a saúde: o nome da distro quando
/// ela entra na distro, ou `None` quando só fala com o serviço do WSL.
pub fn esperar(mut filho: Child, prazo: Duration, rotulo: &str, alvo: Option<&str>) -> Result<Output, String> {
    let inicio = Instant::now();
    let fim = inicio + prazo;
    loop {
        match filho.try_wait() {
            Ok(Some(_)) => {
                let saida = filho.wait_with_output().map_err(|e| e.to_string())?;
                registrar_saida(rotulo, alvo, &saida, inicio.elapsed());
                return Ok(saida);
            }
            Ok(None) if Instant::now() < fim => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = filho.kill();
                let _ = filho.wait();
                let motivo = format!("wsl.exe não respondeu em {} s", prazo.as_secs());
                error!("{rotulo}: {motivo}");
                saude::falhou(alvo, motivo.clone());
                return Err(motivo);
            }
            Err(e) => {
                error!("{rotulo}: {e}");
                return Err(e.to_string());
            }
        }
    }
}

/// Comando que pode levar minutos ou devolver muita saída: esvaziar o cache, percorrer o
/// disco, apagar pastas. A saída é lida enquanto ele roda, porque lida só no fim travaria o
/// `wsl.exe` com o cano cheio. Passar do prazo aqui não conta como o WSL travado: medido, um
/// `sync` com o disco ocupado passou dos 30 s e virou um aviso falso de queda.
pub fn executar_longo(args: &[&str], prazo: Duration) -> Result<Output, String> {
    use std::io::Read;
    let rotulo = rotulo(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>());
    let mut filho = wsl()
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("wsl.exe: {e}"))?;
    let ler = |mut fluxo: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = fluxo.read_to_end(&mut bytes);
            bytes
        })
    };
    let saida = ler(Box::new(filho.stdout.take().expect("stdout pedido como piped")));
    let erro = ler(Box::new(filho.stderr.take().expect("stderr pedido como piped")));
    let inicio = Instant::now();
    let status = loop {
        match filho.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if inicio.elapsed() < prazo => thread::sleep(Duration::from_millis(100)),
            Ok(None) => {
                let _ = filho.kill();
                let _ = filho.wait();
                let motivo = format!("não terminou em {} s", prazo.as_secs());
                warn!("{rotulo}: {motivo}");
                return Err(motivo);
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    info!("{rotulo}: {} ms, saída {:?}", inicio.elapsed().as_millis(), status.code());
    Ok(Output { status, stdout: saida.join().unwrap_or_default(), stderr: erro.join().unwrap_or_default() })
}

pub fn com_prazo(comando: &mut Command, prazo: Duration) -> Result<Output, String> {
    let args: Vec<String> = comando.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    let alvo = args.iter().position(|a| a == "-d").and_then(|i| args.get(i + 1)).cloned();
    let rotulo = rotulo(&args);
    let filho = comando
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            error!("{rotulo}: não iniciou: {e}");
            format!("wsl.exe: {e}")
        })?;
    esperar(filho, prazo, &rotulo, alvo.as_deref())
}

fn rotulo(args: &[String]) -> String {
    let mut r = format!("wsl.exe {}", args.join(" "));
    if r.len() > 160 {
        r.truncate(r.floor_char_boundary(160));
        r.push_str(" (cortado)");
    }
    r
}

/// Saída com código `Wsl/...` é erro do WSL, e com HRESULT (`0x8...`) é falha dele de
/// funcionar, não do pedido: o serviço não respondeu, a VM não subiu. Código nomeado, como
/// o de distro inexistente, é resposta a um pedido que não fazia sentido. Saída diferente
/// de zero sem código é resposta do comando, como um `test -x` que não achou o arquivo.
fn registrar_saida(rotulo: &str, alvo: Option<&str>, saida: &Output, duracao: Duration) {
    let ms = duracao.as_millis();
    let texto = format!("{} {}", decodificar(&saida.stdout), decodificar(&saida.stderr));
    if let Some(codigo) = codigo_de_erro(&texto) {
        let mensagem = texto.split_whitespace().collect::<Vec<_>>().join(" ");
        if falha_de_funcionamento(&codigo) {
            error!("{rotulo}: falhou em {ms} ms com {codigo}: {mensagem}");
            saude::falhou(alvo, codigo);
            return;
        }
        warn!("{rotulo}: recusado em {ms} ms com {codigo}: {mensagem}");
    }
    if duracao > LENTA {
        warn!("{rotulo}: lenta, {ms} ms, saída {:?}", saida.status.code());
    } else {
        debug!("{rotulo}: {ms} ms, saída {:?}", saida.status.code());
    }
    saude::respondeu(alvo);
}

/// O `wsl.exe` fecha as mensagens de erro com algo como "Código de erro:
/// Wsl/Service/0x8007274c". O rótulo vem traduzido conforme o idioma do Windows; o código
/// não.
pub fn codigo_de_erro(texto: &str) -> Option<String> {
    texto
        .split_whitespace()
        .find(|p| p.starts_with("Wsl/"))
        .map(|p| p.trim_end_matches(['.', ',', ';']).to_string())
}

fn falha_de_funcionamento(codigo: &str) -> bool {
    codigo.split('/').any(|parte| parte.to_ascii_lowercase().starts_with("0x8"))
}

/// Acompanha, por alvo, se o WSL está respondendo. Só a virada de estado vira evento, para
/// uma queda de minutos não encher a Atividade com uma linha por chamada.
pub mod saude {
    use super::*;

    struct Queda {
        desde: Instant,
        desde_ms: u64,
        motivo: String,
        falhas: u32,
    }

    /// A queda mais antiga em curso, para a tela que abre no meio dela.
    #[derive(Serialize, Clone)]
    pub struct Situacao {
        pub alvo: Option<String>,
        pub motivo: String,
        pub desde_ms: u64,
    }

    pub enum Mudanca {
        Caiu { alvo: Option<String>, motivo: String },
        Voltou { alvo: Option<String>, segundos: u64, falhas: u32 },
        /// A queda deixou de valer sem prova de volta: o WSL foi desligado ou a distro parou.
        Esquecida,
    }

    /// A chave `None` é o serviço do WSL; as outras, distros.
    static QUEDAS: Mutex<Option<HashMap<Option<String>, Queda>>> = Mutex::new(None);
    static MUDANCAS: Mutex<Vec<Mudanca>> = Mutex::new(Vec::new());

    fn nome(alvo: Option<&str>) -> &str {
        alvo.unwrap_or("serviço")
    }

    pub fn falhou(alvo: Option<&str>, motivo: String) {
        let mut quedas = QUEDAS.lock().unwrap();
        let quedas = quedas.get_or_insert_with(HashMap::new);
        let chave = alvo.map(str::to_string);
        if let Some(q) = quedas.get_mut(&chave) {
            q.falhas += 1;
            q.motivo = motivo;
            return;
        }
        warn!("WSL deixou de responder ({}): {motivo}", nome(alvo));
        MUDANCAS.lock().unwrap().push(Mudanca::Caiu { alvo: chave.clone(), motivo: motivo.clone() });
        let desde_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        quedas.insert(chave, Queda { desde: Instant::now(), desde_ms, motivo, falhas: 1 });
    }

    pub fn respondeu(alvo: Option<&str>) {
        let chave = alvo.map(str::to_string);
        let Some(q) = QUEDAS.lock().unwrap().get_or_insert_with(HashMap::new).remove(&chave) else {
            return;
        };
        let segundos = q.desde.elapsed().as_secs();
        info!(
            "WSL voltou a responder ({}) depois de {segundos} s e {} falha(s); a última: {}",
            nome(alvo),
            q.falhas,
            q.motivo
        );
        MUDANCAS.lock().unwrap().push(Mudanca::Voltou { alvo: chave, segundos, falhas: q.falhas });
    }

    /// Depois de um `--shutdown` o serviço recomeça do zero, e nenhuma queda vale mais.
    pub fn esquecer() {
        let removidas: Vec<Option<String>> = QUEDAS.lock().unwrap().get_or_insert_with(HashMap::new).drain().map(|(alvo, _)| alvo).collect();
        if !removidas.is_empty() {
            info!("quedas esquecidas depois de desligar o WSL: {}", removidas.len());
        }
        MUDANCAS.lock().unwrap().extend(removidas.iter().map(|_| Mudanca::Esquecida));
    }

    /// Distro desligada não está travada: a queda dela sai quando ela some das ligadas.
    pub fn so_das_ligadas(ligadas: &std::collections::HashSet<&str>) {
        let mut quedas = QUEDAS.lock().unwrap();
        let quedas = quedas.get_or_insert_with(HashMap::new);
        let paradas: Vec<Option<String>> =
            quedas.keys().filter(|a| a.as_deref().is_some_and(|d| !ligadas.contains(d))).cloned().collect();
        for alvo in paradas {
            quedas.remove(&alvo);
            info!("queda de {} esquecida: a distro não está mais ligada", nome(alvo.as_deref()));
            MUDANCAS.lock().unwrap().push(Mudanca::Esquecida);
        }
    }

    /// A distro (ou o serviço, com `None`) está em queda agora.
    pub fn tem_queda(alvo: Option<&str>) -> bool {
        QUEDAS.lock().unwrap().as_ref().is_some_and(|q| q.contains_key(&alvo.map(str::to_string)))
    }

    pub fn atual() -> Option<Situacao> {
        let quedas = QUEDAS.lock().unwrap();
        let (alvo, q) = quedas.as_ref()?.iter().min_by_key(|(_, q)| q.desde)?;
        Some(Situacao { alvo: alvo.clone(), motivo: q.motivo.clone(), desde_ms: q.desde_ms })
    }

    /// Entrega as viradas desde a última chamada.
    pub fn mudancas() -> Vec<Mudanca> {
        std::mem::take(&mut *MUDANCAS.lock().unwrap())
    }
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
    executar_com_prazo(args, PRAZO)
}

fn executar_com_prazo(args: &[&str], prazo: Duration) -> Result<String, String> {
    let saida = com_prazo(wsl().args(args), prazo)?;
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
    executar_com_prazo(&["--shutdown"], PRAZO_DESLIGAR).map(|_| ())
}

pub fn versao() -> Result<String, String> {
    executar(&["--version"])
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn acha_o_codigo_na_mensagem_traduzida() {
        let msg = "Uma tentativa de conexão falhou porque o componente conectado não respondeu.\r\nCódigo de erro: Wsl/Service/0x8007274c\r\n";
        assert_eq!(codigo_de_erro(msg).as_deref(), Some("Wsl/Service/0x8007274c"));
        assert_eq!(codigo_de_erro("Nenhuma distribuição em execução."), None);
    }

    #[test]
    fn hresult_e_falha_de_funcionamento_e_codigo_nomeado_nao() {
        assert!(falha_de_funcionamento("Wsl/Service/0x8007274c"));
        assert!(falha_de_funcionamento("Wsl/Service/CreateInstance/CreateVm/HCS/0x80370102"));
        assert!(!falha_de_funcionamento("Wsl/Service/WSL_E_DISTRO_NOT_FOUND"));
    }

    /// Os testes de saúde dividem o estado global do módulo: em paralelo, o que esquece quedas
    /// apagava a do outro, e um em cada ~6 `cargo test` falhava.
    static SAUDE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn so_a_virada_de_estado_vira_mudanca() {
        let _vez = SAUDE.lock().unwrap_or_else(|e| e.into_inner());
        let alvo = Some("distro-do-teste-de-saude");
        saude::falhou(alvo, "primeira".into());
        saude::falhou(alvo, "segunda".into());
        saude::respondeu(alvo);
        saude::respondeu(alvo);
        let minhas: Vec<_> = saude::mudancas()
            .into_iter()
            .filter(|m| match m {
                saude::Mudanca::Caiu { alvo: a, .. } | saude::Mudanca::Voltou { alvo: a, .. } => a.as_deref() == alvo,
                saude::Mudanca::Esquecida => false,
            })
            .collect();
        assert_eq!(minhas.len(), 2);
        assert!(matches!(&minhas[0], saude::Mudanca::Caiu { motivo, .. } if motivo == "primeira"));
        assert!(matches!(&minhas[1], saude::Mudanca::Voltou { falhas: 2, .. }));
    }

    #[test]
    fn queda_de_distro_desligada_e_esquecida() {
        let _vez = SAUDE.lock().unwrap_or_else(|e| e.into_inner());
        let alvo = Some("distro-desligada-do-teste");
        saude::falhou(alvo, "não respondeu".into());
        assert!(saude::tem_queda(alvo));
        saude::so_das_ligadas(&std::collections::HashSet::from(["outra-distro"]));
        assert!(!saude::tem_queda(alvo));
    }
}
