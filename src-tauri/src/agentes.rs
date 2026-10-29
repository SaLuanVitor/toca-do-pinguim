//! Um agente por distro em execução, cada um preso a um `wsl.exe` cujo stdout é lido
//! por uma thread própria.
//!
//! Uma sessão aberta (um `wsl.exe` conectado) mantém a distro ligada, e o agente roda
//! numa. Para o app não mudar o ciclo de vida das distros, o agente sai sozinho quando a
//! sessão dele é a única, e o app espera o prazo de ociosidade do WSL passar antes de pôr
//! outro no lugar. Ao fechar o app, o stdin dos agentes fecha e eles saem também.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use protocolo::{Amostra, Comando, Mensagem};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::atividade::Atividade;
use crate::config::WslConfig;
use crate::historico::Historico;
use crate::wsl::{self, Distro};

/// Gerado por `npm run agente` antes do build do app.
static AGENTE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../target/x86_64-unknown-linux-musl/release/agente"
));

const PAUSA_SUPERVISOR: Duration = Duration::from_secs(2);
const ESPERA_APOS_FALHA: Duration = Duration::from_secs(30);

/// Com a janela minimizada ninguém lê a tela, mas o WebView2 continua processando o
/// que chega. Os agentes desaceleram e as amostras param de ir ao front.
const INTERVALO_PAUSADO_MS: u64 = 5000;

/// Margem sobre o `instanceIdleTimeout`: o WSL leva mais um pouco que o prazo para
/// tirar a distro da lista de execução.
const MARGEM_OCIOSIDADE_MS: i64 = 5000;

#[derive(Serialize, Clone)]
struct AmostraDistro<'a> {
    distro: &'a str,
    amostra: Amostra,
}

#[derive(Serialize, Clone)]
struct RespostaDistro<'a> {
    distro: &'a str,
    ok: bool,
    erro: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct Erro {
    pub distro: Option<String>,
    pub mensagem: String,
}

enum Slot {
    Iniciando,
    Vivo { id: u64, entrada: ChildStdin, _filho: Child },
}

pub struct Agentes {
    slots: Mutex<HashMap<String, Slot>>,
    falhas: Mutex<HashMap<String, Instant>>,
    /// Distros cujo agente saiu por ociosidade, com o instante da saída.
    ociosas: Mutex<HashMap<String, Instant>>,
    /// Sinais enviados e ainda sem resposta, por distro, na ordem de envio: o agente
    /// responde na mesma ordem.
    pendentes: Mutex<HashMap<String, VecDeque<(i32, String, i32)>>>,
    pub historico: Mutex<Historico>,
    pub atividade: Mutex<Atividade>,
    intervalo_ms: AtomicU64,
    espera_ociosidade_ms: AtomicI64,
    pausado: AtomicBool,
    proximo_id: AtomicU64,
}

impl Agentes {
    pub fn novo(dados: &Path) -> Arc<Self> {
        Arc::new(Agentes {
            slots: Mutex::new(HashMap::new()),
            falhas: Mutex::new(HashMap::new()),
            ociosas: Mutex::new(HashMap::new()),
            pendentes: Mutex::new(HashMap::new()),
            historico: Mutex::new(Historico::abrir(dados.join("memoria.csv"))),
            atividade: Mutex::new(Atividade::abrir(dados.join("atividade.jsonl"))),
            intervalo_ms: AtomicU64::new(1000),
            espera_ociosidade_ms: AtomicI64::new(WslConfig::ler().espera_ociosidade_ms()),
            pausado: AtomicBool::new(false),
            proximo_id: AtomicU64::new(1),
        })
    }

    pub fn enviar_sinal(&self, distro: &str, pid: i32, sinal: i32, nome: String) -> Result<(), String> {
        let mut slots = self.slots.lock().unwrap();
        match slots.get_mut(distro) {
            Some(Slot::Vivo { entrada, .. }) => {
                escrever(entrada, &Comando::Sinal { pid, sinal })?;
                self.pendentes
                    .lock()
                    .unwrap()
                    .entry(distro.to_string())
                    .or_default()
                    .push_back((pid, nome, sinal));
                Ok(())
            }
            Some(Slot::Iniciando) => Err(format!("o agente de {distro} ainda está iniciando")),
            None => Err(format!("{distro} não tem agente em execução")),
        }
    }

    pub fn definir_intervalo(&self, ms: u64) {
        self.intervalo_ms.store(ms, Ordering::Relaxed);
        if !self.pausado.load(Ordering::Relaxed) {
            self.repassar_intervalo(ms);
        }
    }

    pub fn pausar(&self, sim: bool) {
        if self.pausado.swap(sim, Ordering::Relaxed) == sim {
            return;
        }
        self.repassar_intervalo(self.intervalo_efetivo());
    }

    fn intervalo_efetivo(&self) -> u64 {
        if self.pausado.load(Ordering::Relaxed) {
            INTERVALO_PAUSADO_MS
        } else {
            self.intervalo_ms.load(Ordering::Relaxed)
        }
    }

    fn repassar_intervalo(&self, ms: u64) {
        let mut slots = self.slots.lock().unwrap();
        for slot in slots.values_mut() {
            if let Slot::Vivo { entrada, .. } = slot {
                let _ = escrever(entrada, &Comando::Intervalo { ms });
            }
        }
    }

    pub fn parar(&self, distro: &str) {
        if let Some(Slot::Vivo { mut entrada, .. }) = self.slots.lock().unwrap().remove(distro) {
            let _ = escrever(&mut entrada, &Comando::Sair);
        }
        self.falhas.lock().unwrap().remove(distro);
        self.ociosas.lock().unwrap().remove(distro);
    }

    pub fn parar_todos(&self) {
        let nomes: Vec<String> = self.slots.lock().unwrap().keys().cloned().collect();
        for nome in nomes {
            self.parar(&nome);
        }
    }

    pub fn registrar(&self, app: &AppHandle, tipo: &str, distro: Option<&str>, texto: String) {
        self.atividade.lock().unwrap().registrar(app, tipo, distro, texto, None);
    }

    /// Sobe o agente de `distro` se ainda não houver um. Não espera: a instalação e a
    /// partida rodam numa thread, e o resultado chega por evento.
    pub fn garantir(self: &Arc<Self>, app: &AppHandle, distro: &str) {
        if let Some(quando) = self.falhas.lock().unwrap().get(distro) {
            if quando.elapsed() < ESPERA_APOS_FALHA {
                return;
            }
        }
        {
            // Subir outro agente aqui reiniciaria a contagem de ociosidade do WSL. Se a
            // distro continua ligada depois do prazo, é porque algo além do app a segura.
            let mut ociosas = self.ociosas.lock().unwrap();
            if let Some(quando) = ociosas.get(distro) {
                let prazo = self.espera_ociosidade_ms.load(Ordering::Relaxed) + MARGEM_OCIOSIDADE_MS;
                if (quando.elapsed().as_millis() as i64) < prazo {
                    return;
                }
                ociosas.remove(distro);
            }
        }
        {
            let mut slots = self.slots.lock().unwrap();
            if slots.contains_key(distro) {
                return;
            }
            slots.insert(distro.to_string(), Slot::Iniciando);
        }
        let (agentes, app, distro) = (Arc::clone(self), app.clone(), distro.to_string());
        thread::spawn(move || agentes.iniciar(app, distro));
    }

    fn iniciar(self: Arc<Self>, app: AppHandle, distro: String) {
        let intervalo = self.intervalo_efetivo();
        // Com `instanceIdleTimeout=-1` o WSL nunca desliga a distro por ociosidade, e o
        // agente não tem por que sair.
        let sair_se_ocioso = self.espera_ociosidade_ms.load(Ordering::Relaxed) >= 0;
        match instalar(&distro).and_then(|caminho| lancar(&distro, &caminho, intervalo, sair_se_ocioso)) {
            Ok((filho, entrada, saida)) => {
                let id = self.proximo_id.fetch_add(1, Ordering::Relaxed);
                self.slots
                    .lock()
                    .unwrap()
                    .insert(distro.clone(), Slot::Vivo { id, entrada, _filho: filho });
                self.falhas.lock().unwrap().remove(&distro);
                thread::spawn(move || self.ler(app, distro, id, saida));
            }
            Err(mensagem) => {
                self.slots.lock().unwrap().remove(&distro);
                self.falhas.lock().unwrap().insert(distro.clone(), Instant::now());
                let _ = app.emit("erro", Erro { distro: Some(distro), mensagem });
            }
        }
    }

    fn ler(self: Arc<Self>, app: AppHandle, distro: String, id: u64, saida: ChildStdout) {
        for linha in BufReader::new(saida).lines() {
            let Ok(linha) = linha else { break };
            match serde_json::from_str::<Mensagem>(&linha) {
                Ok(Mensagem::Amostra(amostra)) => {
                    self.historico.lock().unwrap().registrar(&amostra);
                    if !amostra.kernel.is_empty() {
                        let mut atividade = self.atividade.lock().unwrap();
                        for ev in &amostra.kernel {
                            atividade.kernel(&app, ev);
                        }
                    }
                    if !self.pausado.load(Ordering::Relaxed) {
                        let _ = app.emit("amostra", AmostraDistro { distro: &distro, amostra });
                    }
                }
                Ok(Mensagem::Resposta { ok, erro }) => {
                    let pedido = self.pendentes.lock().unwrap().get_mut(&distro).and_then(VecDeque::pop_front);
                    if let (true, Some((pid, nome, sinal))) = (ok, pedido) {
                        let como = if sinal == 9 { "forçou o encerramento de" } else { "pediu o encerramento de" };
                        self.registrar(&app, "processo", Some(&distro), format!("O app {como} {nome} (PID {pid})"));
                    }
                    let _ = app.emit("resposta", RespostaDistro { distro: &distro, ok, erro });
                }
                Ok(Mensagem::Ocioso) => {
                    self.ociosas.lock().unwrap().insert(distro.clone(), Instant::now());
                    let espera = self.espera_ociosidade_ms.load(Ordering::Relaxed) / 1000;
                    self.registrar(
                        &app,
                        "distro",
                        Some(&distro),
                        format!("{distro} ficou sem atividade; o WSL deve desligá-la em cerca de {espera} s"),
                    );
                    let _ = app.emit("distro_ociosa", &distro);
                }
                Err(e) => eprintln!("[{distro}] linha inválida do agente: {e}"),
            }
        }
        // Um reinício rápido pode já ter posto outro agente no lugar; só sai o slot deste.
        let mut slots = self.slots.lock().unwrap();
        if matches!(slots.get(&distro), Some(Slot::Vivo { id: atual, .. }) if *atual == id) {
            slots.remove(&distro);
        }
        drop(slots);
        self.pendentes.lock().unwrap().remove(&distro);
        let _ = app.emit("agente_parou", &distro);
    }
}

fn escrever(entrada: &mut ChildStdin, cmd: &Comando) -> Result<(), String> {
    let mut linha = serde_json::to_string(cmd).map_err(|e| e.to_string())?;
    linha.push('\n');
    entrada
        .write_all(linha.as_bytes())
        .and_then(|_| entrada.flush())
        .map_err(|e| format!("agente não respondeu: {e}"))
}

/// O nome do arquivo carrega o hash do binário: um app atualizado nunca roda o agente
/// de uma versão anterior que tenha ficado no `/tmp` da distro.
fn caminho_remoto() -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in AGENTE {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("/tmp/wsl-tm/agente-{h:016x}")
}

/// Copia o agente para dentro da distro pelo stdin do `wsl.exe`, sem depender de onde
/// a distro monta os discos do Windows.
fn instalar(distro: &str) -> Result<String, String> {
    let caminho = caminho_remoto();
    let teste = wsl::com_prazo(
        wsl::wsl().args(["-d", distro, "-u", "root", "--exec", "test", "-x", &caminho]),
        wsl::PRAZO,
    )
    .map_err(|e| format!("{distro}: {e}"))?;
    if teste.status.success() {
        return Ok(caminho);
    }

    let script = r#"d="${1%/*}"; mkdir -p "$d" && rm -f "$d"/agente-* && cat > "$1.tmp" && chmod 755 "$1.tmp" && mv -f "$1.tmp" "$1""#;
    let mut filho = wsl::wsl()
        .args(["-d", distro, "-u", "root", "--exec", "sh", "-c", script, "sh", &caminho])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("wsl.exe: {e}"))?;
    // A escrita roda à parte: se a distro travar, ela bloqueia com o cano cheio, e o prazo
    // abaixo mata o `wsl.exe`, o que destrava a escrita com erro. O EOF ao soltar o stdin
    // é o que encerra o `cat`.
    let mut entrada = filho.stdin.take().expect("stdin pedido como piped");
    thread::spawn(move || {
        let _ = entrada.write_all(AGENTE);
    });
    let saida = wsl::esperar(filho, wsl::PRAZO).map_err(|e| format!("{distro}: {e}"))?;
    if saida.status.success() {
        Ok(caminho)
    } else {
        Err(format!(
            "falha ao instalar o agente em {distro}: {}",
            wsl::decodificar(&saida.stderr).trim()
        ))
    }
}

fn lancar(
    distro: &str,
    caminho: &str,
    intervalo_ms: u64,
    sair_se_ocioso: bool,
) -> Result<(Child, ChildStdin, ChildStdout), String> {
    let mut comando = wsl::wsl();
    comando
        .args(["-d", distro, "-u", "root", "--exec", caminho, "--intervalo"])
        .arg(intervalo_ms.to_string());
    if sair_se_ocioso {
        comando.arg("--sair-se-ocioso");
    }
    let mut filho = comando
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("wsl.exe: {e}"))?;
    let entrada = filho.stdin.take().expect("stdin pedido como piped");
    let saida = filho.stdout.take().expect("stdout pedido como piped");
    Ok((filho, entrada, saida))
}

/// Laço de fundo: acompanha quais distros existem e quais estão rodando, sobe agente
/// nas que ligaram, registra quem ligou e desligou, e avisa o front quando a lista muda.
pub fn supervisionar(app: AppHandle, agentes: Arc<Agentes>) {
    thread::spawn(move || {
        let mut ultima: Option<Vec<Distro>> = None;
        loop {
            agentes
                .espera_ociosidade_ms
                .store(WslConfig::ler().espera_ociosidade_ms(), Ordering::Relaxed);
            match wsl::listar() {
                Ok(lista) => {
                    let rodando: HashSet<&str> =
                        lista.iter().filter(|d| d.rodando).map(|d| d.nome.as_str()).collect();
                    if let Some(antes) = &ultima {
                        let antes: HashSet<&str> =
                            antes.iter().filter(|d| d.rodando).map(|d| d.nome.as_str()).collect();
                        for d in rodando.difference(&antes) {
                            agentes.registrar(&app, "distro", Some(d), format!("{d} foi ligada"));
                        }
                        for d in antes.difference(&rodando) {
                            agentes.registrar(&app, "distro", Some(d), format!("{d} foi desligada"));
                        }
                    }
                    agentes.ociosas.lock().unwrap().retain(|d, _| rodando.contains(d.as_str()));
                    if rodando.is_empty() {
                        agentes.atividade.lock().unwrap().vm_parou();
                    }
                    for d in &rodando {
                        agentes.garantir(&app, d);
                    }
                    if ultima.as_ref() != Some(&lista) {
                        let _ = app.emit("distros", &lista);
                        ultima = Some(lista);
                    }
                }
                Err(mensagem) => {
                    let _ = app.emit("erro", Erro { distro: None, mensagem });
                }
            }
            thread::sleep(PAUSA_SUPERVISOR);
        }
    });
}
