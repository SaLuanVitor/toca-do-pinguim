//! Armazenamento: os discos virtuais do WSL, o que o Docker pode liberar e as pastas grandes
//! dentro das distros.
//!
//! Medido numa máquina: o disco do Docker tinha 198,9 GB no Windows e o próprio Docker contava
//! 138 GB recuperáveis; o do Ubuntu tinha 112 GB, com 53 GB num `/tmp` que o WSL não limpa nem
//! ao reiniciar. Pastas do `/tmp` eram montadas por containers rodando, então apagar por idade
//! ou por tamanho sem olhar isso quebraria o container.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::docker;
use crate::wsl;

const PRAZO_DF: Duration = Duration::from_secs(60);
/// Medido com a máquina sem memória: o `docker system df` levou 8 e 14 s, e na primeira
/// leitura da tela, junto com as outras, passou de 60 s.
const PRAZO_DOCKER: Duration = Duration::from_secs(2 * 60);
/// O `du` percorre o disco inteiro da distro. Medido num Ubuntu com 81 GB usados: 172 s com o
/// cache de disco quente; com a VM sem RAM livre, a primeira de duas passadas levou 9 min.
const PRAZO_ANALISE: Duration = Duration::from_secs(20 * 60);
/// Apagar imagens e cache de build de dezenas de GB leva minutos.
const PRAZO_LIMPEZA: Duration = Duration::from_secs(15 * 60);
const ROTULO_PROJETO: &str = "com.docker.compose.project=";

#[derive(Serialize, Clone, Debug)]
pub struct DiscoVirtual {
    /// Distro dona do disco, ou "Docker (dados)".
    pub nome: String,
    pub caminho: String,
    pub tamanho_bytes: u64,
    /// Usado dentro do disco; só com a distro ligada, porque medir não pode ligar nenhuma.
    pub usado_bytes: Option<u64>,
    pub esparso: bool,
    /// Explica um disco que ninguém usa mais, como o de dados do Docker que ficou para trás
    /// quando o Docker Desktop mudou de pasta.
    pub nota: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Categoria {
    /// Images, Containers, Local Volumes ou Build Cache, como o Docker escreve.
    pub tipo: String,
    pub total_bytes: u64,
    pub recuperavel_bytes: u64,
    pub quantidade: u64,
    pub ativos: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Volume {
    pub nome: String,
    pub bytes: u64,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct ResumoDocker {
    pub categorias: Vec<Categoria>,
    /// Volumes que nenhum container usa, do maior para o menor.
    pub volumes_sem_uso: Vec<Volume>,
    /// Containers parados sem projeto do Compose: em geral, sobras de builds.
    pub sobras: u64,
    pub sobras_bytes: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Pasta {
    pub caminho: String,
    pub bytes: u64,
    pub modificado_ms: Option<u64>,
    /// Container rodando que monta esta pasta.
    pub em_uso_por: Option<String>,
    pub apagavel: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Analise {
    pub distro: String,
    pub raiz: Vec<Pasta>,
    pub tmp: Vec<Pasta>,
    pub caches: Vec<Pasta>,
}

// --- Discos virtuais ---------------------------------------------------------

struct Registrada {
    nome: String,
    base: PathBuf,
}

/// Distros registradas no WSL, lidas do registro do usuário, sem chamar o `wsl.exe`.
fn registradas() -> Vec<Registrada> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_SZ,
    };
    let largo = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let raiz = largo(r"Software\Microsoft\Windows\CurrentVersion\Lxss");
    let mut chave: HKEY = std::ptr::null_mut();
    // SAFETY: as chamadas recebem textos terminados em zero e buffers com o tamanho informado;
    // a chave aberta é fechada no fim.
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, raiz.as_ptr(), 0, KEY_READ, &mut chave) != 0 {
            return Vec::new();
        }
        let ler = |sub: &[u16], valor: &str| -> Option<String> {
            let valor = largo(valor);
            let mut buffer = vec![0u16; 1024];
            let mut tamanho = (buffer.len() * 2) as u32;
            let ok = RegGetValueW(chave, sub.as_ptr(), valor.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut tamanho) == 0;
            ok.then(|| {
                let fim = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
                String::from_utf16_lossy(&buffer[..fim])
            })
        };
        let mut lista = Vec::new();
        for i in 0.. {
            let mut nome = vec![0u16; 256];
            let mut n = nome.len() as u32;
            if RegEnumKeyExW(chave, i, nome.as_mut_ptr(), &mut n, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) != 0 {
                break;
            }
            let sub: Vec<u16> = nome[..n as usize].iter().copied().chain([0]).collect();
            if let (Some(distro), Some(base)) = (ler(&sub, "DistributionName"), ler(&sub, "BasePath")) {
                lista.push(Registrada { nome: distro, base: PathBuf::from(base.trim_start_matches(r"\\?\")) });
            }
        }
        RegCloseKey(chave);
        lista
    }
}

fn esparso(caminho: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_SPARSE_FILE: u32 = 0x200;
    std::fs::metadata(caminho).is_ok_and(|m| m.file_attributes() & FILE_ATTRIBUTE_SPARSE_FILE != 0)
}

/// Pasta onde o Docker Desktop guarda os discos, se ela foi mudada nas configurações dele.
fn pasta_do_docker() -> Option<PathBuf> {
    let arquivo = PathBuf::from(std::env::var_os("APPDATA")?).join(r"Docker\settings-store.json");
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(arquivo).ok()?).ok()?;
    json.get("CustomWslDistroDir")?.as_str().filter(|s| !s.is_empty()).map(PathBuf::from)
}

/// `df -P` porque a distro do Docker tem o `df` do BusyBox, sem as opções do GNU; o formato
/// POSIX é o mesmo nos dois, com o usado na terceira coluna.
fn usado(distro: &str, ponto: &str) -> Option<u64> {
    let saida = wsl::executar_longo(&["-d", distro, "-u", "root", "--exec", "df", "-P", "-B1", ponto], PRAZO_DF).ok()?;
    usado_do_df(&wsl::decodificar(&saida.stdout))
}

fn usado_do_df(texto: &str) -> Option<u64> {
    texto.lines().nth(1)?.split_whitespace().nth(2)?.parse().ok()
}

pub fn discos(ligadas: &[String]) -> Vec<DiscoVirtual> {
    let ligada = |d: &str| ligadas.iter().any(|l| l == d);
    let mut lista = Vec::new();
    for r in registradas() {
        let Ok(entradas) = std::fs::read_dir(&r.base) else { continue };
        for e in entradas.flatten().filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("vhdx"))) {
            let tamanho = e.metadata().map_or(0, |m| m.len());
            lista.push(DiscoVirtual {
                usado_bytes: if ligada(&r.nome) && r.nome != "docker-desktop" { usado(&r.nome, "/") } else { None },
                nome: r.nome.clone(),
                caminho: e.path().display().to_string(),
                tamanho_bytes: tamanho,
                esparso: esparso(&e.path()),
                nota: None,
            });
        }
    }
    let padrao = std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join(r"Docker\wsl\disk\docker_data.vhdx"));
    let atual = pasta_do_docker().map(|p| p.join(r"disk\docker_data.vhdx")).or(padrao.clone());
    if let Some(atual) = atual.filter(|p| p.is_file()) {
        lista.push(DiscoVirtual {
            nome: "Docker (dados)".to_string(),
            caminho: atual.display().to_string(),
            tamanho_bytes: std::fs::metadata(&atual).map_or(0, |m| m.len()),
            usado_bytes: if ligada("docker-desktop") { usado("docker-desktop", "/mnt/docker-desktop-disk") } else { None },
            esparso: esparso(&atual),
            nota: None,
        });
        if let Some(antigo) = padrao.filter(|p| p.is_file() && *p != atual) {
            lista.push(DiscoVirtual {
                nome: "Docker (dados, antigo)".to_string(),
                caminho: antigo.display().to_string(),
                tamanho_bytes: std::fs::metadata(&antigo).map_or(0, |m| m.len()),
                usado_bytes: None,
                esparso: esparso(&antigo),
                nota: Some(format!(
                    "O Docker Desktop usa a pasta {}; este arquivo ficou da pasta anterior e ele não o abre mais. Pode ser apagado com o Docker Desktop fechado.",
                    atual.parent().and_then(Path::parent).map_or(String::new(), |p| p.display().to_string())
                )),
            });
        }
    }
    lista
}

// --- Docker ------------------------------------------------------------------

/// "90.87GB (72%)" e "20.77GB" viram bytes.
fn bytes_do_docker(texto: &str) -> u64 {
    docker::tamanho(texto.split_whitespace().next().unwrap_or("")).unwrap_or(0)
}

#[derive(Deserialize)]
struct LinhaDf {
    #[serde(rename = "Type")]
    tipo: String,
    #[serde(rename = "TotalCount")]
    total: String,
    #[serde(rename = "Active")]
    ativos: String,
    #[serde(rename = "Size")]
    tamanho: String,
    #[serde(rename = "Reclaimable")]
    recuperavel: String,
}

fn ler_categorias(texto: &str) -> Vec<Categoria> {
    texto
        .lines()
        .filter_map(|l| serde_json::from_str::<LinhaDf>(l).ok())
        .map(|l| Categoria {
            tipo: l.tipo,
            total_bytes: bytes_do_docker(&l.tamanho),
            recuperavel_bytes: bytes_do_docker(&l.recuperavel),
            quantidade: l.total.parse().unwrap_or(0),
            ativos: l.ativos.parse().unwrap_or(0),
        })
        .collect()
}

/// O `df -v` traz cada volume, container e imagem; dele saem os volumes sem uso, as sobras de
/// build e as pastas que os containers rodando montam.
struct Detalhe {
    volumes_sem_uso: Vec<Volume>,
    sobras: u64,
    sobras_bytes: u64,
    /// (distro, pasta montada, container)
    montagens: Vec<(String, String, String)>,
}

fn ler_detalhe(texto: &str) -> Detalhe {
    let json: serde_json::Value = serde_json::from_str(texto).unwrap_or_default();
    let texto_de = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let mut volumes: Vec<Volume> = json["Volumes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| texto_de(v, "Links") == "0")
        .map(|v| Volume { nome: texto_de(v, "Name"), bytes: bytes_do_docker(&texto_de(v, "Size")) })
        .collect();
    volumes.sort_by_key(|v| std::cmp::Reverse(v.bytes));
    let (mut sobras, mut sobras_bytes, mut montagens) = (0, 0, Vec::new());
    for c in json["Containers"].as_array().into_iter().flatten() {
        let rotulos = texto_de(c, "Labels");
        if texto_de(c, "State") == "running" {
            montagens.extend(montagens_do_container(&texto_de(c, "Names"), &rotulos));
        } else if !rotulos.contains(ROTULO_PROJETO) {
            sobras += 1;
            sobras_bytes += bytes_do_docker(&texto_de(c, "Size"));
        }
    }
    Detalhe { volumes_sem_uso: volumes, sobras, sobras_bytes, montagens }
}

/// O Docker Desktop marca as pastas de uma distro montadas num container com rótulos como
/// `desktop.docker.io/mounts/0/Source=/tmp/x` e `desktop.docker.io/wsl-distro=Ubuntu`.
fn montagens_do_container(nome: &str, rotulos: &str) -> Vec<(String, String, String)> {
    let mut distro = String::new();
    let mut fontes = Vec::new();
    for par in rotulos.split(',') {
        let Some((chave, valor)) = par.split_once('=') else { continue };
        if chave == "desktop.docker.io/wsl-distro" {
            distro = valor.to_string();
        } else if chave.starts_with("desktop.docker.io/mounts/") && chave.ends_with("/Source") && valor.starts_with('/') {
            fontes.push(valor.to_string());
        }
    }
    fontes.into_iter().map(|f| (distro.clone(), f, nome.to_string())).collect()
}

pub fn resumo_docker() -> Result<(ResumoDocker, Vec<(String, String, String)>), String> {
    let categorias = ler_categorias(&docker::executar(&["system", "df", "--format", "{{json .}}"], PRAZO_DOCKER)?);
    let detalhe = ler_detalhe(&docker::executar(&["system", "df", "-v", "--format", "{{json .}}"], PRAZO_ANALISE)?);
    Ok((
        ResumoDocker { categorias, volumes_sem_uso: detalhe.volumes_sem_uso, sobras: detalhe.sobras, sobras_bytes: detalhe.sobras_bytes },
        detalhe.montagens,
    ))
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum LimpezaDocker {
    /// Containers parados sem projeto do Compose.
    Sobras,
    /// Todos os containers parados, inclusive os de projetos.
    Parados,
    ImagensSemUso,
    CacheDeBuild,
    Volumes,
}

fn volume_valido(nome: &str) -> bool {
    !nome.is_empty() && nome.len() <= 255 && nome.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric()) && nome.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

/// Devolve o que o Docker diz ter liberado ("Total reclaimed space: 1.2GB").
pub fn limpar_docker(qual: LimpezaDocker, volumes: &[String], permitidos: &[Volume]) -> Result<u64, String> {
    let filtro = format!("label!={}", ROTULO_PROJETO.trim_end_matches('='));
    let saida = match qual {
        LimpezaDocker::Sobras => docker::executar(&["container", "prune", "-f", "--filter", &filtro], PRAZO_LIMPEZA)?,
        LimpezaDocker::Parados => docker::executar(&["container", "prune", "-f"], PRAZO_LIMPEZA)?,
        LimpezaDocker::ImagensSemUso => docker::executar(&["image", "prune", "-a", "-f"], PRAZO_LIMPEZA)?,
        LimpezaDocker::CacheDeBuild => docker::executar(&["builder", "prune", "-a", "-f"], PRAZO_LIMPEZA)?,
        LimpezaDocker::Volumes => {
            // Só volumes da última leitura sem uso, com nome de volume de verdade: um nome como
            // "--all" não vira opção do comando.
            if volumes.is_empty() || !volumes.iter().all(|v| volume_valido(v) && permitidos.iter().any(|p| &p.nome == v)) {
                return Err("lista de volumes inválida ou desatualizada; atualize a tela".to_string());
            }
            let liberado: u64 = permitidos.iter().filter(|p| volumes.contains(&p.nome)).map(|p| p.bytes).sum();
            let mut args = vec!["volume", "rm", "--"];
            args.extend(volumes.iter().map(String::as_str));
            docker::executar(&args, PRAZO_LIMPEZA)?;
            return Ok(liberado);
        }
    };
    Ok(saida
        .lines()
        .find_map(|l| l.strip_prefix("Total reclaimed space:").or_else(|| l.strip_prefix("Total:")))
        .map_or(0, |v| bytes_do_docker(v.trim())))
}

// --- Pastas dentro da distro ----------------------------------------------------

/// Pastas que guardam cache e podem ser esvaziadas: apagar o conteúdo só faz o programa baixar
/// ou gerar de novo o que precisar.
const CACHES: [&str; 6] = ["/root/.cache", "/home/*/.cache", "/home/*/.npm", "/home/*/.cargo/registry/cache", "/var/cache/apt/archives", "/var/tmp"];

/// Saída em linhas `R|T|C|M<TAB>MB<TAB>mtime<TAB>caminho`, para o app não depender de JSON
/// dentro da distro. Um `du` só cobre a raiz e o `/tmp`: com um para cada, o `/tmp` era lido
/// duas vezes, e as duas passadas somadas passaram de 10 min. As linhas `M` trazem a data de
/// cada entrada do `/tmp`.
const SCRIPT_ANALISE: &str = r#"
du -x -a -d2 -B1M / 2>/dev/null | awk -F'\t' '$2 ~ /^\/[^\/]+$/ { print "R\t" $1 "\t0\t" $2 } $2 ~ /^\/tmp\/[^\/]+$/ { print "T\t" $1 "\t0\t" $2 }'
for p in /tmp/* /tmp/.[!.]*; do [ -e "$p" ] || continue; printf 'M\t0\t%s\t%s\n' "$(stat -c %Y "$p" 2>/dev/null)" "$p"; done
for p in __CACHES__; do [ -d "$p" ] || continue; mb=$(du -xsm "$p" 2>/dev/null | cut -f1); printf 'C\t%s\t0\t%s\n' "${mb:-0}" "$p"; done
"#;
const MAIORES_DA_RAIZ: usize = 12;

fn ler_analise(distro: &str, texto: &str, montagens: &[(String, String, String)]) -> Analise {
    let mut a = Analise { distro: distro.to_string(), ..Default::default() };
    let mut datas = std::collections::HashMap::new();
    for linha in texto.lines() {
        let mut c = linha.splitn(4, '\t');
        let (Some(tipo), Some(mb), Some(m), Some(caminho)) = (c.next(), c.next(), c.next(), c.next()) else { continue };
        let bytes = mb.trim().parse::<u64>().unwrap_or(0) * 1024 * 1024;
        let modificado_ms = m.trim().parse::<u64>().ok().filter(|&s| s > 0).map(|s| s * 1000);
        let em_uso_por = montagens
            .iter()
            .find(|(d, fonte, _)| d == distro && (fonte == caminho || fonte.starts_with(&format!("{caminho}/"))))
            .map(|(_, _, nome)| nome.clone());
        let pasta = |apagavel| Pasta { caminho: caminho.to_string(), bytes, modificado_ms, apagavel, em_uso_por: em_uso_por.clone() };
        match tipo {
            "R" => a.raiz.push(pasta(false)),
            "T" => a.tmp.push(pasta(em_uso_por.is_none())),
            "C" => a.caches.push(pasta(em_uso_por.is_none())),
            "M" => {
                datas.insert(caminho.to_string(), modificado_ms);
            }
            _ => {}
        }
    }
    for p in &mut a.tmp {
        if let Some(&m) = datas.get(&p.caminho) {
            p.modificado_ms = p.modificado_ms.or(m);
        }
    }
    a.raiz.retain(|p| p.caminho != "/");
    a.raiz.sort_by_key(|p| std::cmp::Reverse(p.bytes));
    a.raiz.truncate(MAIORES_DA_RAIZ);
    a.tmp.sort_by_key(|p| std::cmp::Reverse(p.bytes));
    a.tmp.truncate(40);
    a.caches.sort_by_key(|p| std::cmp::Reverse(p.bytes));
    a
}

pub fn analisar(distro: &str, montagens: &[(String, String, String)]) -> Result<Analise, String> {
    let script = SCRIPT_ANALISE.replace("__CACHES__", &CACHES.join(" "));
    // O `timeout` de dentro mata o `du` junto: matar só o wsl.exe deixava a varredura rodando.
    let prazo = (PRAZO_ANALISE.as_secs() - 10).to_string();
    let saida = wsl::executar_longo(&["-d", distro, "-u", "root", "--exec", "timeout", "-k", "5", &prazo, "sh", "-c", &script], PRAZO_ANALISE)?;
    Ok(ler_analise(distro, &wsl::decodificar(&saida.stdout), montagens))
}

/// Confere cada caminho contra a última análise da distro: só entra o que ela marcou como
/// apagável. Entrada do `/tmp` sai inteira; pasta de cache é esvaziada, e a pasta fica.
pub fn apagar(distro: &str, caminhos: &[String], analise: &Analise) -> Result<u64, String> {
    let achar = |lista: &[Pasta], c: &String| lista.iter().find(|p| &p.caminho == c && p.apagavel).cloned();
    let mut tmp = Vec::new();
    let mut caches = Vec::new();
    for c in caminhos {
        if c.contains("/../") || c.ends_with("/..") {
            return Err(format!("caminho recusado: {c}"));
        }
        match (achar(&analise.tmp, c), achar(&analise.caches, c)) {
            (Some(p), _) if p.caminho.starts_with("/tmp/") => tmp.push(p),
            (_, Some(p)) => caches.push(p),
            _ => return Err(format!("{c} não está entre o que pode ser apagado; analise de novo")),
        }
    }
    if !tmp.is_empty() {
        let mut args: Vec<&str> = vec!["-d", distro, "-u", "root", "--exec", "rm", "-rf", "--"];
        args.extend(tmp.iter().map(|p| p.caminho.as_str()));
        executar_na_distro(&args)?;
    }
    for p in &caches {
        executar_na_distro(&["-d", distro, "-u", "root", "--exec", "find", &p.caminho, "-xdev", "-mindepth", "1", "-delete"])?;
    }
    Ok(tmp.iter().chain(&caches).map(|p| p.bytes).sum())
}

fn executar_na_distro(args: &[&str]) -> Result<(), String> {
    let saida = wsl::executar_longo(args, PRAZO_LIMPEZA)?;
    if saida.status.success() {
        Ok(())
    } else {
        Err(wsl::decodificar(&saida.stderr).trim().to_string())
    }
}

/// Limpeza programada do `/tmp`: entradas sem mudança há mais de `dias`, fora as montadas por
/// container. As pastas em uso vão como argumentos e o script as pula.
pub fn limpar_tmp_antigo(distro: &str, dias: u32, em_uso: &[String]) -> Result<u64, String> {
    let script = r#"d="$1"; shift; total=0
for p in /tmp/* /tmp/.[!.]*; do
  [ -e "$p" ] || continue
  for u in "$@"; do case "$u" in "$p"|"$p"/*) continue 2;; esac; done
  if [ -n "$(find "$p" -maxdepth 0 -mtime +"$d" 2>/dev/null)" ]; then
    mb=$(du -xsm "$p" 2>/dev/null | cut -f1); rm -rf -- "$p" && total=$((total + ${mb:-0}))
  fi
done
echo "$total""#;
    let dias = dias.to_string();
    let mut args: Vec<&str> = vec!["-d", distro, "-u", "root", "--exec", "sh", "-c", script, "sh", &dias];
    args.extend(em_uso.iter().map(String::as_str));
    let saida = wsl::executar_longo(&args, PRAZO_LIMPEZA)?;
    Ok(wsl::decodificar(&saida.stdout).trim().parse::<u64>().unwrap_or(0) * 1024 * 1024)
}

pub fn em_uso_na_distro(distro: &str, montagens: &[(String, String, String)]) -> Vec<String> {
    let mut v: Vec<String> = montagens.iter().filter(|(d, _, _)| d == distro).map(|(_, f, _)| f.clone()).collect();
    v.sort();
    v.dedup();
    v
}

#[cfg(test)]
mod testes {
    use super::*;

    const MB: u64 = 1024 * 1024;

    #[test]
    fn le_o_usado_do_df_do_gnu_e_do_busybox() {
        let gnu = "Filesystem     1-blocks        Used   Available Capacity Mounted on
/dev/sdd 1081101176832 89335382016 939781836800 9% /
";
        let busybox = "Filesystem           1-blocks      Used Available Use% Mounted on
/dev/sdf             1081101176832 189936672768 836182454272  19% /mnt/docker-desktop-disk
";
        assert_eq!(usado_do_df(gnu), Some(89_335_382_016));
        assert_eq!(usado_do_df(busybox), Some(189_936_672_768));
        assert_eq!(usado_do_df("df: não achou"), None);
    }

    #[test]
    fn le_as_categorias_do_docker_system_df() {
        let texto = r#"{"Active":"47","Reclaimable":"90.87GB (72%)","Size":"125.7GB","TotalCount":"117","Type":"Images"}
{"Active":"0","Reclaimable":"20.77GB","Size":"48.34GB","TotalCount":"715","Type":"Build Cache"}"#;
        let c = ler_categorias(texto);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].tipo.as_str(), c[0].quantidade, c[0].ativos), ("Images", 117, 47));
        assert_eq!(c[0].recuperavel_bytes, (90.87 * 1e9) as u64);
        assert_eq!(c[1].total_bytes, (48.34 * 1e9) as u64);
    }

    #[test]
    fn detalhe_separa_volumes_sem_uso_sobras_e_montagens() {
        let texto = r#"{"Volumes":[
            {"Name":"banco","Links":"1","Size":"2GB"},
            {"Name":"velho","Links":"0","Size":"300MB"},
            {"Name":"maior","Links":"0","Size":"1.5GB"}],
          "Containers":[
            {"Names":"app","State":"running","Size":"16kB","Labels":"desktop.docker.io/mounts/0/Source=/tmp/obra/src,desktop.docker.io/mounts/0/Target=/app,desktop.docker.io/wsl-distro=Ubuntu"},
            {"Names":"runner-build","State":"exited","Size":"500MB","Labels":"x=y"},
            {"Names":"loja-db","State":"exited","Size":"20MB","Labels":"com.docker.compose.project=loja"}]}"#;
        let d = ler_detalhe(texto);
        assert_eq!(d.volumes_sem_uso.iter().map(|v| v.nome.as_str()).collect::<Vec<_>>(), ["maior", "velho"]);
        assert_eq!((d.sobras, d.sobras_bytes), (1, 500_000_000));
        assert_eq!(d.montagens, vec![("Ubuntu".to_string(), "/tmp/obra/src".to_string(), "app".to_string())]);
    }

    #[test]
    fn pasta_montada_por_container_nao_e_apagavel() {
        let montagens = vec![("Ubuntu".to_string(), "/tmp/obra/src".to_string(), "app".to_string())];
        let texto = "R\t53000\t0\t/tmp\nR\t17000\t0\t/home\nT\t30000\t0\t/tmp/obra\nT\t11000\t0\t/tmp/solta\nC\t4400\t0\t/root/.cache\nM\t0\t1793200000\t/tmp/obra\nM\t0\t1793100000\t/tmp/solta\n";
        let a = ler_analise("Ubuntu", texto, &montagens);
        assert_eq!(a.raiz.len(), 2);
        assert!(!a.raiz[0].apagavel);
        let obra = a.tmp.iter().find(|p| p.caminho == "/tmp/obra").unwrap();
        assert_eq!((obra.em_uso_por.as_deref(), obra.apagavel), (Some("app"), false));
        let solta = a.tmp.iter().find(|p| p.caminho == "/tmp/solta").unwrap();
        assert!(solta.apagavel && solta.bytes == 11000 * MB && solta.modificado_ms == Some(1_793_100_000_000));
        // Montagem em outra distro não conta.
        let b = ler_analise("Debian", texto, &montagens);
        assert!(b.tmp.iter().all(|p| p.apagavel));
    }

    #[test]
    fn raiz_fica_com_as_maiores_sem_o_total() {
        let mut texto = String::from("R\t81000\t0\t/\n");
        for i in 0..20 {
            texto.push_str(&format!("R\t{i}\t0\t/pasta{i}\n"));
        }
        let a = ler_analise("Ubuntu", &texto, &[]);
        assert_eq!(a.raiz.len(), MAIORES_DA_RAIZ);
        assert_eq!(a.raiz[0].caminho, "/pasta19");
        assert!(a.raiz.iter().all(|p| p.caminho != "/"));
    }

    /// O script roda de verdade num sh com `du` e `awk`, sobre uma árvore montada aqui; só onde
    /// existe um sh POSIX (no CI do Linux). Confere o filtro de profundidade do awk.
    #[cfg(unix)]
    #[test]
    fn awk_separa_raiz_e_tmp() {
        let linhas = "10\t/tmp/a\n20\t/tmp\n5\t/home/x\n30\t/home\n90\t/\n1\t/tmp/a/b\n";
        let filtro = SCRIPT_ANALISE.lines().find(|l| l.starts_with("du ")).unwrap();
        let awk = &filtro[filtro.find("awk").unwrap()..];
        let saida = std::process::Command::new("sh")
            .args(["-c", &format!("printf '{}' | {awk}", linhas.replace('\t', "\\t").replace('\n', "\\n"))])
            .output()
            .unwrap();
        let texto = String::from_utf8(saida.stdout).unwrap();
        assert_eq!(texto, "T\t10\t0\t/tmp/a\nR\t20\t0\t/tmp\nR\t30\t0\t/home\n");
    }

    #[test]
    fn apagar_so_aceita_o_que_a_analise_marcou() {
        let montagens = vec![("Ubuntu".to_string(), "/tmp/obra".to_string(), "app".to_string())];
        let a = ler_analise("Ubuntu", "T\t1\t0\t/tmp/obra\nT\t1\t0\t/tmp/solta\nR\t1\t0\t/home\n", &montagens);
        assert!(apagar("Ubuntu", &["/tmp/obra".to_string()], &a).is_err());
        assert!(apagar("Ubuntu", &["/home".to_string()], &a).is_err());
        assert!(apagar("Ubuntu", &["/etc".to_string()], &a).is_err());
        assert!(apagar("Ubuntu", &["/tmp/solta/../../etc".to_string()], &a).is_err());
    }

    #[test]
    fn volume_com_nome_de_opcao_e_recusado() {
        let permitidos = vec![Volume { nome: "velho".to_string(), bytes: 1 }];
        assert!(limpar_docker(LimpezaDocker::Volumes, &["--all".to_string()], &permitidos).is_err());
        assert!(limpar_docker(LimpezaDocker::Volumes, &["outro".to_string()], &permitidos).is_err());
        assert!(limpar_docker(LimpezaDocker::Volumes, &[], &permitidos).is_err());
        assert!(volume_valido("farol-sql-journal-f1a4") && !volume_valido("-x") && !volume_valido("a b"));
    }
}
