//! Containers do Docker Desktop, pela CLI `docker` do Windows.
//!
//! Os containers rodam na mesma VM das distros e são, medido numa máquina com quatro
//! projetos de pé, o que mais ocupa a memória dela. A CLI só é chamada com a distro
//! `docker-desktop` ligada: sem o Docker de pé ela só devolveria erro.

use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Medido: listar, inspecionar e ler a memória de 19 containers leva de 4 a 5 s juntos.
const PRAZO_CONSULTA: Duration = Duration::from_secs(30);
/// Parar espera cada container sair, até 10 s cada, em paralelo.
const PRAZO_ACAO: Duration = Duration::from_secs(120);
const ROTULO_PROJETO: &str = "com.docker.compose.project";

/// Um container que reinicia isto de vezes dentro da janela está preso num ciclo: sobe,
/// cai e volta, e cada volta recarrega o programa inteiro.
const REINICIOS_EM_CICLO: usize = 3;
const JANELA_CICLO: Duration = Duration::from_secs(5 * 60);

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Conteiner {
    pub id: String,
    pub nome: String,
    /// Projeto do Docker Compose; `None` para container avulso.
    pub projeto: Option<String>,
    /// Como o Docker diz: `running`, `restarting`, `exited`, `paused`...
    pub estado: String,
    pub memoria_bytes: u64,
    /// Limite de memória do container; 0 quando não tem, e ele pode usar a VM toda.
    pub limite_bytes: u64,
    pub reinicios: u64,
    pub em_ciclo: bool,
    /// Resultado do healthcheck (`healthy`, `unhealthy`, `starting`), quando a imagem ou o
    /// compose declara um.
    pub saude: Option<String>,
}

impl Conteiner {
    pub fn sem_resposta(&self) -> bool {
        self.saude.as_deref() == Some("unhealthy")
    }

    /// Acima disto do limite, o kernel encerra o container na próxima falta de memória.
    pub fn perto_do_limite(&self) -> bool {
        self.limite_bytes > 0 && self.memoria_bytes as f64 >= self.limite_bytes as f64 * LIMITE_PERTO
    }
}

const LIMITE_PERTO: f64 = 0.9;

struct Inspecao {
    reinicios: u64,
    saude: Option<String>,
    limite_bytes: u64,
}

/// Guarda, por container, a última contagem de reinícios e quando ela subiu, para
/// separar o container que reiniciou uma vez ontem do que está caindo agora.
#[derive(Default)]
pub struct Vigia {
    reinicios: HashMap<String, (u64, VecDeque<Instant>)>,
}

impl Vigia {
    fn observar(&mut self, id: &str, contagem: u64, agora: Instant) -> bool {
        let (anterior, subidas) = self.reinicios.entry(id.to_string()).or_insert((contagem, VecDeque::new()));
        for _ in *anterior..contagem.min(*anterior + REINICIOS_EM_CICLO as u64) {
            subidas.push_back(agora);
        }
        *anterior = contagem;
        while subidas.front().is_some_and(|t| agora.duration_since(*t) > JANELA_CICLO) {
            subidas.pop_front();
        }
        subidas.len() >= REINICIOS_EM_CICLO
    }
}

/// A saída é lida enquanto o processo roda: a lista de um Docker com uma centena de
/// containers passa do que cabe no cano, e lida só no fim travaria o `docker`.
fn executar(args: &[&str], prazo: Duration) -> Result<String, String> {
    let mut filho = Command::new("docker")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => "a CLI docker não está instalada".to_string(),
            _ => format!("docker: {e}"),
        })?;
    let ler = |mut fluxo: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut texto = String::new();
            let _ = fluxo.read_to_string(&mut texto);
            texto
        })
    };
    let saida = ler(Box::new(filho.stdout.take().expect("stdout pedido como piped")));
    let erro = ler(Box::new(filho.stderr.take().expect("stderr pedido como piped")));
    let fim = Instant::now() + prazo;
    let status = loop {
        match filho.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < fim => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = filho.kill();
                let _ = filho.wait();
                return Err(format!("docker {} não respondeu em {} s", args[0], prazo.as_secs()));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    let (saida, erro) = (saida.join().unwrap_or_default(), erro.join().unwrap_or_default());
    if status.success() {
        Ok(saida)
    } else {
        Err(erro.split_whitespace().collect::<Vec<_>>().join(" "))
    }
}

/// Projetos do Compose aparecem mesmo parados, para poderem ser ligados de volta; avulsos,
/// só rodando. Parados e avulsos são, em geral, sobras de builds.
pub fn listar(vigia: &mut Vigia) -> Result<Vec<Conteiner>, String> {
    let formato = format!("{{{{.ID}}}}\t{{{{.Names}}}}\t{{{{.State}}}}\t{{{{.Label \"{ROTULO_PROJETO}\"}}}}");
    let mut lista: Vec<Conteiner> = ler_ps(&executar(&["ps", "-a", "--no-trunc", "--format", &formato], PRAZO_CONSULTA)?)
        .into_iter()
        .filter(|c| c.projeto.is_some() || ativo(&c.estado))
        .collect();

    let ativos: Vec<&str> = lista.iter().filter(|c| ativo(&c.estado)).map(|c| c.id.as_str()).collect();
    if !ativos.is_empty() {
        // `.State` chega ao template como mapa, e `.State.Health` num container sem
        // healthcheck derruba o `inspect` inteiro ("map has no entry for key"); o `index`
        // devolve vazio.
        let formato = "{{.Id}}\t{{.RestartCount}}\t{{with index .State \"Health\"}}{{.Status}}{{end}}\t{{.HostConfig.Memory}}";
        let mut args = vec!["inspect", "--format", formato];
        args.extend(&ativos);
        let inspecoes = ler_pares(&executar(&args, PRAZO_CONSULTA)?, ler_inspecao);
        let memoria = ler_pares(
            &executar(&["stats", "--no-stream", "--no-trunc", "--format", "{{.ID}}\t{{.MemUsage}}"], PRAZO_CONSULTA)?,
            |v| tamanho(v.split(" / ").next().unwrap_or("")),
        );
        let agora = Instant::now();
        for c in &mut lista {
            if let Some(i) = inspecoes.get(&c.id) {
                c.reinicios = i.reinicios;
                c.saude = i.saude.clone();
                c.limite_bytes = i.limite_bytes;
            }
            c.memoria_bytes = memoria.get(&c.id).copied().unwrap_or(0);
            c.em_ciclo = c.estado == "restarting" || vigia.observar(&c.id, c.reinicios, agora);
        }
    }
    vigia.reinicios.retain(|id, _| lista.iter().any(|c| &c.id == id));
    Ok(lista)
}

fn ativo(estado: &str) -> bool {
    matches!(estado, "running" | "restarting")
}

pub fn parar(ids: &[String]) -> Result<(), String> {
    agir("stop", ids)
}

pub fn iniciar(ids: &[String]) -> Result<(), String> {
    agir("start", ids)
}

pub fn reiniciar(ids: &[String]) -> Result<(), String> {
    agir("restart", ids)
}

/// Os ids vêm do front; conferir o formato impede que algo como `--help` vire opção.
fn agir(verbo: &str, ids: &[String]) -> Result<(), String> {
    if ids.is_empty() || !ids.iter().all(|id| id_valido(id)) {
        return Err("lista de containers inválida".to_string());
    }
    let mut args = vec![verbo];
    args.extend(ids.iter().map(String::as_str));
    executar(&args, PRAZO_ACAO).map(|_| ())
}

fn id_valido(id: &str) -> bool {
    (12..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit())
}

fn ler_ps(texto: &str) -> Vec<Conteiner> {
    texto
        .lines()
        .filter_map(|linha| {
            let mut campos = linha.trim_end_matches('\r').split('\t');
            let (id, nome, estado) = (campos.next()?, campos.next()?, campos.next()?);
            let projeto = campos.next().map(str::trim).filter(|p| !p.is_empty()).map(str::to_string);
            id_valido(id).then(|| Conteiner {
                id: id.to_string(),
                nome: nome.to_string(),
                projeto,
                estado: estado.to_string(),
                memoria_bytes: 0,
                limite_bytes: 0,
                reinicios: 0,
                em_ciclo: false,
                saude: None,
            })
        })
        .collect()
}

/// `reinicios<TAB>saúde<TAB>limite`, com a saúde vazia quando não há healthcheck.
fn ler_inspecao(texto: &str) -> Option<Inspecao> {
    let mut campos = texto.split('\t');
    let reinicios = campos.next()?.trim().parse().ok()?;
    let saude = campos.next().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let limite_bytes = campos.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    Some(Inspecao { reinicios, saude, limite_bytes })
}

/// Linhas `id<TAB>valor`. O `inspect` devolve o id completo; o `stats` também, com
/// `--no-trunc`.
fn ler_pares<T>(texto: &str, valor: impl Fn(&str) -> Option<T>) -> HashMap<String, T> {
    texto
        .lines()
        .filter_map(|linha| {
            let (id, v) = linha.trim_end_matches('\r').split_once('\t')?;
            Some((id.to_string(), valor(v.trim())?))
        })
        .collect()
}

/// Tamanhos como o `docker stats` escreve: `136.3MiB`, `1.722GiB`, `0B`, `1.2kB`.
fn tamanho(texto: &str) -> Option<u64> {
    let fim = texto.find(|c: char| c.is_ascii_alphabetic())?;
    let (numero, unidade) = texto.split_at(fim);
    let fator: f64 = match unidade {
        "B" => 1.0,
        "kB" | "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((numero.trim().parse::<f64>().ok()? * fator) as u64)
}

#[cfg(test)]
mod testes {
    use super::*;

    const ID_A: &str = "5ca458c9f26a0000000000000000000000000000000000000000000000000001";
    const ID_B: &str = "68b20e9d8fd40000000000000000000000000000000000000000000000000002";

    #[test]
    fn le_tamanhos_do_docker_stats() {
        assert_eq!(tamanho("0B"), Some(0));
        assert_eq!(tamanho("512KiB"), Some(512 * 1024));
        assert_eq!(tamanho("136.3MiB"), Some((136.3 * 1024.0 * 1024.0) as u64));
        assert_eq!(tamanho("1.722GiB"), Some((1.722 * 1024.0 * 1024.0 * 1024.0) as u64));
        assert_eq!(tamanho("1.2kB"), Some(1200));
        assert_eq!(tamanho("--"), None);
        assert_eq!(tamanho("12XB"), None);
    }

    #[test]
    fn le_a_lista_com_e_sem_projeto() {
        let texto = format!("{ID_A}\tsigc-app\trunning\tprefeitura-sigc-semge\r\n{ID_B}\tgitlab-runner\trunning\t\nlixo\tsem\tid\n");
        let lista = ler_ps(&texto);
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].projeto.as_deref(), Some("prefeitura-sigc-semge"));
        assert_eq!(lista[1].projeto, None);
        assert_eq!(lista[1].estado, "running");
    }

    #[test]
    fn le_reinicios_saude_e_limite_do_inspect() {
        let texto = format!("{ID_A}\t20\tunhealthy\t536870912\n{ID_B}\t0\t\t0\n");
        let lido = ler_pares(&texto, ler_inspecao);
        let a = &lido[ID_A];
        assert_eq!((a.reinicios, a.saude.as_deref(), a.limite_bytes), (20, Some("unhealthy"), 512 << 20));
        let b = &lido[ID_B];
        assert_eq!((b.reinicios, b.saude.as_deref(), b.limite_bytes), (0, None, 0));
    }

    #[test]
    fn perto_do_limite_so_com_limite_definido() {
        let mut c = ler_ps(&format!("{ID_A}\tleitor-pdf\trunning\tsigc\n")).remove(0);
        c.memoria_bytes = 470 << 20;
        assert!(!c.perto_do_limite());
        c.limite_bytes = 512 << 20;
        assert!(c.perto_do_limite());
        c.memoria_bytes = 400 << 20;
        assert!(!c.perto_do_limite());
    }

    #[test]
    fn id_so_passa_em_hexadecimal() {
        assert!(id_valido(ID_A));
        assert!(id_valido("5ca458c9f26a"));
        assert!(!id_valido("--help"));
        assert!(!id_valido("5ca458c9f26"));
        assert!(!id_valido("5ca458c9f26a 68b2"));
        assert!(agir("stop", &["--all".to_string()]).is_err());
        assert!(agir("stop", &[]).is_err());
    }

    #[test]
    fn ciclo_so_com_reinicios_seguidos_dentro_da_janela() {
        let mut v = Vigia::default();
        let t0 = Instant::now();
        // Reinícios antigos, já contados na primeira vez que o app vê o container, não contam.
        assert!(!v.observar("a", 20, t0));
        assert!(!v.observar("a", 21, t0 + Duration::from_secs(15)));
        assert!(!v.observar("a", 22, t0 + Duration::from_secs(30)));
        assert!(v.observar("a", 23, t0 + Duration::from_secs(45)));
        // Parou de reiniciar: sai do ciclo quando os reinícios saem da janela.
        assert!(v.observar("a", 23, t0 + Duration::from_secs(60)));
        assert!(!v.observar("a", 23, t0 + Duration::from_secs(45) + JANELA_CICLO + Duration::from_secs(1)));
        // Vários reinícios entre duas leituras contam juntos.
        assert!(v.observar("b", 0, t0) == false && v.observar("b", 5, t0 + Duration::from_secs(15)));
    }
}
