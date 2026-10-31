mod agentes;
mod armazenamento;
mod atalho;
mod atividade;
mod bandeja;
mod atualizacao;
mod cache;
mod compactar;
mod config;
mod configuracao;
mod docker;
mod historico;
mod recursos;
mod registro;
mod travados;
mod wsl;

use std::path::Path;
use std::sync::Arc;

use agentes::Agentes;
use log::{info, warn};
use atividade::Evento;
use config::WslConfig;
use recursos::Recursos;
use serde::Serialize;
use tauri::{Manager, RunEvent, State, WindowEvent};
use wsl::Distro;

pub(crate) type Estado<'a> = State<'a, Arc<Agentes>>;

const GIB: u64 = 1 << 30;

/// Os comandos que chamam `wsl.exe` levam de dezenas de milissegundos a segundos;
/// rodam fora da thread principal para a janela não congelar.
pub(crate) async fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

pub(crate) fn ram_host() -> u64 {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: estrutura zerada com `dwLength` preenchido, como a API exige.
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut m) } != 0 { m.ullTotalPhys } else { 0 }
}

#[derive(Serialize)]
struct ResumoMemoria {
    limite_bytes: u64,
    /// Falso quando o `.wslconfig` não define `memory` e vale o padrão do WSL.
    limite_configurado: bool,
    ram_host_bytes: u64,
    minutos_de_dados: usize,
    pico_bytes: u64,
    sugestao_bytes: Option<u64>,
}

#[tauri::command]
async fn listar_distros() -> Result<Vec<Distro>, String> {
    bloqueante(wsl::listar).await
}

#[tauri::command]
async fn iniciar_distro(app: tauri::AppHandle, estado: Estado<'_>, nome: String) -> Result<(), String> {
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        info!("pedido: iniciar {nome}");
        wsl::iniciar(&nome)?;
        agentes.acordar(&nome);
        agentes.garantir(&app, &nome);
        Ok(())
    })
    .await
}

#[tauri::command]
async fn terminar_distro(estado: Estado<'_>, nome: String) -> Result<(), String> {
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        info!("pedido: terminar {nome}");
        agentes.parar(&nome);
        wsl::terminar(&nome)
    })
    .await
}

#[tauri::command]
async fn reiniciar_distro(app: tauri::AppHandle, estado: Estado<'_>, nome: String) -> Result<(), String> {
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        info!("pedido: reiniciar {nome}");
        agentes.parar(&nome);
        wsl::terminar(&nome)?;
        wsl::iniciar(&nome)?;
        agentes.garantir(&app, &nome);
        Ok(())
    })
    .await
}

#[tauri::command]
async fn desligar_wsl(app: tauri::AppHandle, estado: Estado<'_>) -> Result<(), String> {
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        info!("pedido: desligar o WSL");
        let resultado = desligar_sem_religar(&agentes);
        soltar_depois(&agentes);
        resultado?;
        agentes.registrar(&app, "wsl", None, "O app desligou o WSL".to_string());
        Ok(())
    })
    .await
}

#[tauri::command]
fn ler_segundo_plano(recursos: State<'_, Arc<Recursos>>) -> bandeja::SegundoPlano {
    bandeja::SegundoPlano {
        fechar_para_bandeja: recursos.preferencias.lock().unwrap().fechar_para_bandeja,
        iniciar_com_windows: bandeja::iniciar_com_windows(),
    }
}

#[tauri::command]
fn definir_segundo_plano(recursos: State<'_, Arc<Recursos>>, fechar_para_bandeja: bool, iniciar_com_windows: bool) -> Result<(), String> {
    recursos.definir_fechar_para_bandeja(fechar_para_bandeja)?;
    if iniciar_com_windows != bandeja::iniciar_com_windows() {
        bandeja::definir_iniciar_com_windows(iniciar_com_windows)?;
    }
    info!("segundo plano: fechar para a bandeja {fechar_para_bandeja}, abrir com o Windows {iniciar_com_windows}");
    Ok(())
}

/// `parar` apaga a marca de agente que saiu por ociosidade, e o supervisor, com uma leitura
/// da lista anterior ao `--shutdown`, subiria o agente de novo e religaria a VM. Os agentes
/// ficam segurados até `soltar_depois`.
fn desligar_sem_religar(agentes: &Agentes) -> Result<(), String> {
    agentes.segurar(true);
    agentes.parar_todos();
    wsl::desligar()?;
    // Desligado, o WSL recomeça do zero: a queda registrada e a espera crescente entre
    // tentativas de subir agente ficaram para trás, e a próxima distro ligada é lida na hora.
    wsl::saude::esquecer();
    agentes.esquecer_falhas();
    Ok(())
}

/// Solta os agentes quando o supervisor já leu a lista de depois do desligamento.
fn soltar_depois(agentes: &Arc<Agentes>) {
    let agentes = Arc::clone(agentes);
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        agentes.segurar(false);
    });
}

#[tauri::command]
fn enviar_sinal(estado: Estado<'_>, distro: String, pid: i32, sinal: i32, nome: String) -> Result<(), String> {
    info!("pedido: sinal {sinal} para {nome} (PID {pid}) em {distro}");
    estado.enviar_sinal(&distro, pid, sinal, nome)
}

#[tauri::command]
fn definir_intervalo(estado: Estado<'_>, ms: u64) {
    estado.definir_intervalo(ms.max(250));
}

#[derive(Serialize)]
struct EstadoRecursos {
    memoria: recursos::MemoriaWindows,
    docker: recursos::Docker,
    saude: Option<wsl::saude::Situacao>,
    devolucao: Option<cache::Devolucao>,
    devolver_cache_automatico: bool,
    gatilho: cache::Gatilho,
}

/// O que as threads de fundo já sabem, para a tela que abre depois do último evento.
#[tauri::command]
fn estado_recursos(recursos: State<'_, Arc<Recursos>>) -> EstadoRecursos {
    // Uma trava só: a guarda de um `lock()` dentro do literal vive até o fim dele, e travar o
    // mesmo Mutex de novo ali trava o comando para sempre.
    let prefs = recursos.preferencias.lock().unwrap().clone();
    EstadoRecursos {
        memoria: recursos.memoria.lock().unwrap().clone(),
        docker: recursos.docker.lock().unwrap().clone(),
        saude: wsl::saude::atual(),
        devolucao: recursos.devolucao.lock().unwrap().clone(),
        devolver_cache_automatico: prefs.devolver_cache_automatico,
        gatilho: prefs.gatilho,
    }
}

#[tauri::command]
async fn devolver_cache(app: tauri::AppHandle, agentes: Estado<'_>, recursos: State<'_, Arc<Recursos>>) -> Result<(), String> {
    let (agentes, recursos) = (Arc::clone(&agentes), Arc::clone(&recursos));
    info!("pedido: devolver o cache da VM");
    bloqueante(move || cache::devolver(app, agentes, recursos, false)).await
}

#[derive(Serialize)]
struct Discos {
    discos: Vec<armazenamento::DiscoVirtual>,
    limpeza: cache::LimpezaProgramada,
    distros_ligadas: Vec<String>,
    /// Todas as registradas, ligadas ou não, para a análise de pastas.
    distros: Vec<String>,
}

const DISTRO_DOCKER: &str = "docker-desktop";

/// Os discos, a limpeza programada e as distros: o tamanho vem do arquivo e o ocupado de um `df`
/// por distro ligada, então volta em menos de um segundo. O Docker vem à parte.
#[tauri::command]
async fn ler_discos(agentes: Estado<'_>, recursos: State<'_, Arc<Recursos>>) -> Result<Discos, String> {
    let (agentes, recursos) = (Arc::clone(&agentes), Arc::clone(&recursos));
    bloqueante(move || {
        let ligadas = agentes.ligadas();
        Ok(Discos {
            discos: armazenamento::discos(&ligadas),
            limpeza: recursos.preferencias.lock().unwrap().limpeza.clone(),
            distros_ligadas: ligadas.into_iter().filter(|d| d != DISTRO_DOCKER).collect(),
            distros: armazenamento::distros_registradas(),
        })
    })
    .await
}

/// Sem `forcar`, devolve na hora a última leitura guardada, se houver; a tela mostra e pede uma
/// nova se ela estiver velha. O `docker system df` leva segundos (medido: ~5 s por consulta).
#[tauri::command]
async fn ler_espaco_docker(agentes: Estado<'_>, recursos: State<'_, Arc<Recursos>>, forcar: bool) -> Result<armazenamento::EspacoDocker, String> {
    // "Docker desligado" guardado não vale se ele ligou depois.
    let guardada = recursos.espaco_docker.lock().unwrap().clone();
    if let Some(g) = guardada.filter(|g| !forcar && (g.docker.is_some() || g.erro.is_some() || !agentes.rodando(DISTRO_DOCKER))) {
        return Ok(g);
    }
    let (agentes, recursos) = (Arc::clone(&agentes), Arc::clone(&recursos));
    bloqueante(move || Ok(recursos.ler_espaco_docker(agentes.rodando(DISTRO_DOCKER)))).await
}

/// Lê o espaço do Docker uma vez, assim que ele estiver ligado, para a aba Disco já abrir com
/// ele. Desiste depois de 10 min sem Docker: quem não tem Docker não paga nada.
fn antecipar_espaco_docker(agentes: Arc<Agentes>, recursos: Arc<Recursos>) {
    std::thread::spawn(move || {
        for _ in 0..60 {
            std::thread::sleep(std::time::Duration::from_secs(10));
            if recursos.espaco_docker.lock().unwrap().is_some() {
                return;
            }
            if agentes.rodando(DISTRO_DOCKER) {
                let lido = recursos.ler_espaco_docker(true);
                info!("espaço do Docker lido de antemão: {}", if lido.erro.is_some() { "com erro" } else { "ok" });
                return;
            }
        }
    });
}

/// Distro desligada só é analisada com `ligar`, que a tela pede depois de confirmar. O app não
/// a mantém ligada: terminada a análise, o WSL a desliga sozinho quando ela fica ociosa.
#[tauri::command]
async fn analisar_pastas(
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    distro: String,
    ligar: bool,
) -> Result<armazenamento::Analise, String> {
    if distro.starts_with(DISTRO_DOCKER) || !armazenamento::distros_registradas().contains(&distro) {
        return Err(format!("{distro} não é uma distro que dê para analisar"));
    }
    if !ligar && !agentes.rodando(&distro) {
        return Err(format!("{distro} está desligada; confirme para ligá-la e analisar"));
    }
    let (agentes, recursos) = (Arc::clone(&agentes), Arc::clone(&recursos));
    bloqueante(move || {
        info!("pedido: analisar as pastas de {distro}");
        let montagens = if agentes.rodando(DISTRO_DOCKER) { armazenamento::resumo_docker().map(|(_, m)| m).unwrap_or_default() } else { Vec::new() };
        let mut analise = armazenamento::analisar(&distro, &montagens)?;
        analise.analisado_ms = agora_ms();
        let guardada = analise.clone();
        recursos.guardar_analise(move |a| {
            a.insert(distro, guardada);
        });
        Ok(analise)
    })
    .await
}

const ANALISE_VALE_PARA_APAGAR_MS: u64 = 30 * 60 * 1000;

fn agora_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// A última análise guardada da distro, para a tela reabrir com ela sem ler de novo.
#[tauri::command]
fn ultima_analise(recursos: State<'_, Arc<Recursos>>, distro: String) -> Option<armazenamento::Analise> {
    recursos.analises.lock().unwrap().get(&distro).cloned()
}

#[tauri::command]
async fn apagar_pastas(
    app: tauri::AppHandle,
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    distro: String,
    caminhos: Vec<String>,
) -> Result<u64, String> {
    let analise = recursos.analises.lock().unwrap().get(&distro).cloned().ok_or("analise a distro antes de apagar")?;
    // A análise guardada serve para mostrar; para apagar, ela precisa ser recente. Uma pasta do
    // /tmp recriada depois dela pode ter trabalho novo com o mesmo nome.
    if agora_ms().saturating_sub(analise.analisado_ms) > ANALISE_VALE_PARA_APAGAR_MS {
        return Err("a análise tem mais de 30 min; analise de novo antes de apagar".to_string());
    }
    let (agentes, recursos) = (Arc::clone(&agentes), Arc::clone(&recursos));
    bloqueante(move || {
        info!("pedido: apagar {} item(ns) de {distro}", caminhos.len());
        let liberado = armazenamento::apagar(&distro, &caminhos, &analise)?;
        recursos.guardar_analise(|analises| {
            if let Some(a) = analises.get_mut(&distro) {
                a.tmp.retain(|p| !caminhos.contains(&p.caminho));
                for p in a.caches.iter_mut().filter(|p| caminhos.contains(&p.caminho)) {
                    p.bytes = 0;
                }
            }
        });
        agentes.registrar(
            &app,
            "armazenamento",
            Some(&distro),
            format!("O app apagou {} GB em {distro}: {}", recursos::gb(liberado), caminhos.join(", ")),
        );
        Ok(liberado)
    })
    .await
}

#[derive(Serialize)]
struct Compactacao {
    antes_bytes: u64,
    depois_bytes: u64,
}

static COMPACTANDO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Desliga o WSL inteiro e compacta um disco que a aba Disco listou. O `fstrim` antes, com a
/// distro ainda ligada, marca como livre o que o ext4 não marcou ao apagar.
#[tauri::command]
async fn compactar_disco(app: tauri::AppHandle, agentes: Estado<'_>, caminho: String) -> Result<Compactacao, String> {
    use std::sync::atomic::Ordering;
    let agentes = Arc::clone(&agentes);
    bloqueante(move || {
        let disco = armazenamento::discos(&[])
            .into_iter()
            .find(|d| d.caminho == caminho)
            .ok_or("esse disco não está mais na lista; atualize a aba Disco")?;
        if !disco.compactavel {
            return Err(format!("o disco de {} não precisa ser compactado", disco.nome));
        }
        if compactar::docker_aberto() {
            return Err("feche o Docker Desktop antes de compactar: ele religa o WSL e prende o disco".to_string());
        }
        if COMPACTANDO.swap(true, Ordering::SeqCst) {
            return Err("já há um disco sendo compactado".to_string());
        }
        let resultado = (|| {
            info!("pedido: compactar {}", disco.caminho);
            if disco.nome != DISTRO_DOCKER && agentes.rodando(&disco.nome) {
                // Por `sh -c`: o `--exec` direto não tem o /usr/sbin no PATH e não acha o fstrim.
                match wsl::executar_longo(&["-d", &disco.nome, "-u", "root", "--exec", "sh", "-c", "fstrim -v /"], std::time::Duration::from_secs(5 * 60)) {
                    Ok(s) => info!(
                        "fstrim em {}: {:?} {} {}",
                        disco.nome,
                        s.status.code(),
                        wsl::decodificar(&s.stdout).trim(),
                        wsl::decodificar(&s.stderr).trim()
                    ),
                    Err(e) => warn!("fstrim em {}: {e}", disco.nome),
                }
            }
            desligar_sem_religar(&agentes)?;
            agentes.registrar(&app, "wsl", None, format!("O app desligou o WSL para compactar o disco de {}", disco.nome));
            let (antes, depois) = compactar::compactar(Path::new(&disco.caminho))?;
            agentes.registrar(
                &app,
                "armazenamento",
                Some(&disco.nome),
                format!(
                    "O app compactou o disco de {}: de {} GB para {} GB no Windows",
                    disco.nome,
                    recursos::gb(antes),
                    recursos::gb(depois)
                ),
            );
            Ok(Compactacao { antes_bytes: antes, depois_bytes: depois })
        })();
        soltar_depois(&agentes);
        COMPACTANDO.store(false, Ordering::SeqCst);
        resultado
    })
    .await
}

#[tauri::command]
async fn limpar_parados_do_projeto(
    app: tauri::AppHandle,
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    projeto: String,
) -> Result<u64, String> {
    if !agentes.rodando(DISTRO_DOCKER) {
        return Err("o Docker Desktop não está ligado".to_string());
    }
    let conhecidos = recursos.projetos_docker.lock().unwrap().clone();
    let agentes = Arc::clone(&agentes);
    bloqueante(move || {
        info!("pedido: apagar os containers parados do projeto {projeto}");
        let liberado = armazenamento::limpar_parados_do_projeto(&projeto, &conhecidos)?;
        agentes.registrar(
            &app,
            "armazenamento",
            Some(DISTRO_DOCKER),
            format!("O app apagou os containers parados do projeto {projeto}: {} GB liberados", recursos::gb(liberado)),
        );
        Ok(liberado)
    })
    .await
}

#[tauri::command]
async fn limpar_docker(
    app: tauri::AppHandle,
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    qual: armazenamento::LimpezaDocker,
    volumes: Vec<String>,
) -> Result<u64, String> {
    if !agentes.rodando(DISTRO_DOCKER) {
        return Err("o Docker Desktop não está ligado".to_string());
    }
    let permitidos = recursos.volumes_sem_uso.lock().unwrap().clone();
    let agentes = Arc::clone(&agentes);
    bloqueante(move || {
        info!("pedido: limpar o Docker ({qual:?})");
        let liberado = armazenamento::limpar_docker(qual, &volumes, &permitidos)?;
        let oque = match qual {
            armazenamento::LimpezaDocker::Sobras => "os containers parados de builds".to_string(),
            armazenamento::LimpezaDocker::Parados => "todos os containers parados".to_string(),
            armazenamento::LimpezaDocker::ImagensSemUso => "as imagens sem uso".to_string(),
            armazenamento::LimpezaDocker::CacheDeBuild => "o cache de build".to_string(),
            armazenamento::LimpezaDocker::Volumes => format!("{} volume(s): {}", volumes.len(), volumes.join(", ")),
        };
        agentes.registrar(&app, "armazenamento", Some(DISTRO_DOCKER), format!("O app apagou {oque} do Docker: {} GB liberados", recursos::gb(liberado)));
        Ok(liberado)
    })
    .await
}

#[tauri::command]
fn definir_limpeza(recursos: State<'_, Arc<Recursos>>, limpeza: cache::LimpezaProgramada) -> Result<(), String> {
    if limpeza.tmp_dias.is_some_and(|d| d < 1) {
        return Err("o /tmp precisa de pelo menos 1 dia".to_string());
    }
    info!("limpeza programada: {limpeza:?}");
    recursos.definir_limpeza(limpeza)
}

/// Liga ou desliga a devolução automática e define quando ela dispara.
#[tauri::command]
fn definir_devolucao(
    recursos: State<'_, Arc<Recursos>>,
    automatico: bool,
    minimo_livre_pct: u8,
    cache_maximo_gb: Option<f64>,
) -> Result<(), String> {
    if !(1..=50).contains(&minimo_livre_pct) {
        return Err("a memória livre do gatilho precisa ficar entre 1% e 50%".to_string());
    }
    let ram = ram_host();
    let cache_maximo_bytes = match cache_maximo_gb {
        None => None,
        Some(gb) if gb >= 0.5 && gb * GIB as f64 <= ram as f64 => Some((gb * GIB as f64) as u64),
        Some(_) => return Err(format!("o cache máximo precisa ficar entre 0,5 GB e {} GB", ram / GIB)),
    };
    info!("devolver o cache sozinho: {automatico}, abaixo de {minimo_livre_pct}% livre ou cache acima de {cache_maximo_gb:?} GB");
    recursos.definir_devolucao(automatico, cache::Gatilho { minimo_livre_pct, cache_maximo_bytes })
}

enum AcaoDocker {
    Parar,
    Ligar,
    Reiniciar,
}

async fn agir_em_conteineres(
    app: tauri::AppHandle,
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    ids: Vec<String>,
    rotulo: String,
    acao: AcaoDocker,
) -> Result<(), String> {
    recursos.conferir_ids(&ids)?;
    let (agentes, recursos) = (Arc::clone(&agentes), Arc::clone(&recursos));
    bloqueante(move || {
        let (verbo, feito, executar): (&str, &str, fn(&[String]) -> Result<(), String>) = match acao {
            AcaoDocker::Parar => ("parar", "parou", docker::parar),
            AcaoDocker::Ligar => ("ligar", "ligou", docker::iniciar),
            AcaoDocker::Reiniciar => ("reiniciar", "reiniciou", docker::reiniciar),
        };
        info!("pedido: {verbo} {rotulo} ({} container(s))", ids.len());
        let resultado = executar(&ids);
        recursos.pedir_atualizacao();
        resultado?;
        agentes.registrar(&app, "docker", Some("docker-desktop"), format!("O app {feito} {rotulo} ({} container(s))", ids.len()));
        Ok(())
    })
    .await
}

#[tauri::command]
async fn parar_conteineres(
    app: tauri::AppHandle,
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    ids: Vec<String>,
    rotulo: String,
) -> Result<(), String> {
    agir_em_conteineres(app, agentes, recursos, ids, rotulo, AcaoDocker::Parar).await
}

#[tauri::command]
async fn iniciar_conteineres(
    app: tauri::AppHandle,
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    ids: Vec<String>,
    rotulo: String,
) -> Result<(), String> {
    agir_em_conteineres(app, agentes, recursos, ids, rotulo, AcaoDocker::Ligar).await
}

#[tauri::command]
async fn reiniciar_conteineres(
    app: tauri::AppHandle,
    agentes: Estado<'_>,
    recursos: State<'_, Arc<Recursos>>,
    ids: Vec<String>,
    rotulo: String,
) -> Result<(), String> {
    agir_em_conteineres(app, agentes, recursos, ids, rotulo, AcaoDocker::Reiniciar).await
}

#[tauri::command]
fn listar_atividade(estado: Estado<'_>) -> Vec<Evento> {
    estado.atividade.lock().unwrap().listar()
}

#[tauri::command]
fn resumo_memoria(estado: Estado<'_>) -> ResumoMemoria {
    let ram = ram_host();
    let configurado = WslConfig::ler()
        .obter("wsl2", "memory")
        .and_then(|v| config::tamanho_em_bytes(&v));
    let r = estado.historico.lock().unwrap().resumo(ram);
    ResumoMemoria {
        // Sem `memory` no arquivo, o WSL usa metade da RAM do Windows.
        limite_bytes: configurado.unwrap_or(ram / 2),
        limite_configurado: configurado.is_some(),
        ram_host_bytes: ram,
        minutos_de_dados: r.minutos_de_dados,
        pico_bytes: r.pico_bytes,
        sugestao_bytes: r.sugestao_bytes,
    }
}

/// Grava `memory` no `.wslconfig`. A mudança só vale quando a VM reinicia; com
/// `reiniciar`, o app desliga o WSL na hora, senão ela vale no próximo início.
#[tauri::command]
async fn definir_limite_memoria(
    app: tauri::AppHandle,
    estado: Estado<'_>,
    gb: u64,
    reiniciar: bool,
) -> Result<(), String> {
    let ram_gb = ram_host() / GIB;
    if gb < 1 || gb > ram_gb {
        return Err(format!("o limite precisa ficar entre 1 e {ram_gb} GB"));
    }
    let agentes = Arc::clone(&estado);
    bloqueante(move || {
        let mut cfg = WslConfig::ler();
        let antes = cfg.obter("wsl2", "memory").unwrap_or_else(|| "padrão (metade da RAM)".to_string());
        cfg.definir("wsl2", "memory", &format!("{gb}GB"));
        cfg.gravar().map_err(|e| format!("não foi possível gravar o .wslconfig: {e}"))?;
        info!("limite de memória do WSL: {antes} para {gb}GB (reiniciar: {reiniciar})");
        agentes.registrar(&app, "config", None, format!("Limite de memória do WSL: {antes} para {gb}GB"));
        if reiniciar {
            agentes.parar_todos();
            wsl::desligar()?;
            agentes.registrar(&app, "wsl", None, "O app desligou o WSL para aplicar a configuração".to_string());
        }
        Ok(())
    })
    .await
}

/// Identificador de quando o app se chamava Gerenciador de WSL; a pasta de dados vem dele.
const IDENTIFICADOR_ANTIGO: &str = "com.luan.wsltm";
const ARQUIVOS_DE_DADOS: [&str; 2] = ["memoria.csv", "atividade.jsonl"];

/// Copia o histórico da pasta do nome antigo, uma vez: arquivo que já existe na pasta nova
/// não é sobrescrito. A pasta antiga fica intacta, para quem voltar à versão anterior.
fn trazer_dados_antigos(dados: &Path) {
    let Some(antiga) = dados.parent().map(|p| p.join(IDENTIFICADOR_ANTIGO)) else { return };
    for nome in ARQUIVOS_DE_DADOS {
        let (de, para) = (antiga.join(nome), dados.join(nome));
        if de.is_file() && !para.exists() {
            if let Err(e) = std::fs::copy(&de, &para) {
                warn!("não copiei {} da pasta antiga: {e}", de.display());
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Uma segunda abertura (pela tecla de atalho, pelo menu) só traz esta janela para
        // frente: duas instâncias subiriam dois agentes em cada distro.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| bandeja::mostrar(app)))
        // Logo depois, para o log já pegar o que os outros plugins e o setup registram.
        .plugin(registro::plugin())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            info!("Toca do Pinguim {} iniciado", app.package_info().version);
            // Versão do WSL e do kernel, para o log servir de diagnóstico sem perguntar.
            std::thread::spawn(|| match wsl::versao() {
                Ok(v) => info!("wsl --version: {}", v.split_whitespace().collect::<Vec<_>>().join(" ")),
                Err(e) => warn!("wsl --version falhou: {e}"),
            });
            let dados = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dados)?;
            trazer_dados_antigos(&dados);
            let agentes = Agentes::novo(&dados);
            app.manage(Arc::clone(&agentes));
            let recursos = Arc::new(Recursos::novo(&dados));
            app.manage(Arc::clone(&recursos));
            antecipar_espaco_docker(Arc::clone(&agentes), Arc::clone(&recursos));
            recursos::vigiar(app.handle().clone(), Arc::clone(&agentes), recursos);
            agentes::supervisionar(app.handle().clone(), agentes);
            bandeja::montar(app)?;
            // A janela nasce escondida (tauri.conf.json) e só aparece quando não foi o Windows
            // que abriu o app em segundo plano; assim ela não pisca na abertura.
            if std::env::args().any(|a| a == bandeja::ARGUMENTO) {
                info!("aberto em segundo plano, na área de notificação");
                bandeja::esconder(app.handle());
            } else {
                bandeja::mostrar(app.handle());
            }
            Ok(())
        })
        .on_window_event(|janela, evento| match evento {
            // Minimizar chega como redimensionamento; restaurar também.
            WindowEvent::Resized(_) => {
                let minimizada = janela.is_minimized().unwrap_or(false);
                janela.state::<Arc<Agentes>>().pausar(minimizada);
            }
            WindowEvent::CloseRequested { api, .. } => {
                let recursos = janela.state::<Arc<Recursos>>();
                if recursos.preferencias.lock().unwrap().fechar_para_bandeja {
                    api.prevent_close();
                    bandeja::esconder(janela.app_handle());
                    if recursos.primeira_vez_na_bandeja() {
                        recursos::notificar(
                            janela.app_handle(),
                            "A Toca do Pinguim continua aberta",
                            "Ela fica na área de notificação, ao lado do relógio. Para sair de vez, use Sair no menu do ícone.",
                        );
                    }
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            listar_distros,
            iniciar_distro,
            terminar_distro,
            reiniciar_distro,
            desligar_wsl,
            enviar_sinal,
            definir_intervalo,
            listar_atividade,
            resumo_memoria,
            estado_recursos,
            parar_conteineres,
            iniciar_conteineres,
            reiniciar_conteineres,
            devolver_cache,
            definir_devolucao,
            ler_discos,
            ler_espaco_docker,
            analisar_pastas,
            ultima_analise,
            apagar_pastas,
            compactar_disco,
            limpar_docker,
            limpar_parados_do_projeto,
            definir_limpeza,
            definir_limite_memoria,
            configuracao::ler_wslconfig,
            configuracao::gravar_wslconfig,
            configuracao::ler_wslconf,
            configuracao::gravar_wslconf,
            configuracao::limites_da_maquina,
            atualizacao::procurar_atualizacao,
            atualizacao::instalar_atualizacao,
            atualizacao::abrir_pagina_da_versao,
            registro::ler_log,
            registro::abrir_pasta_de_logs,
            atalho::ler_atalho_teclado,
            atalho::definir_atalho_teclado,
            ler_segundo_plano,
            definir_segundo_plano,
        ])
        .build(tauri::generate_context!())
        .expect("falha ao montar o app")
        .run(|app, evento| {
            if let RunEvent::Exit = evento {
                if let Some(agentes) = app.try_state::<Arc<Agentes>>() {
                    agentes.parar_todos();
                }
            }
        });
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn traz_da_pasta_antiga_sem_sobrescrever() {
        let raiz = std::env::temp_dir().join(format!("toca-migracao-{}", std::process::id()));
        let (antiga, nova) = (raiz.join(IDENTIFICADOR_ANTIGO), raiz.join("com.luan.tocadopinguim"));
        std::fs::create_dir_all(&antiga).unwrap();
        std::fs::create_dir_all(&nova).unwrap();
        std::fs::write(antiga.join("memoria.csv"), "antigo").unwrap();
        std::fs::write(antiga.join("atividade.jsonl"), "antigo").unwrap();
        std::fs::write(nova.join("atividade.jsonl"), "novo").unwrap();

        trazer_dados_antigos(&nova);

        assert_eq!(std::fs::read_to_string(nova.join("memoria.csv")).unwrap(), "antigo");
        assert_eq!(std::fs::read_to_string(nova.join("atividade.jsonl")).unwrap(), "novo");
        assert!(antiga.join("memoria.csv").is_file());
        std::fs::remove_dir_all(&raiz).unwrap();
    }
}
