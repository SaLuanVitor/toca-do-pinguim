//! "Sugerir otimização": monta um plano a partir do que o app mediu (as ocorrências dos últimos
//! dias, o espaço do Docker e os containers) e deixa a pessoa marcar o que fazer. O plano só usa
//! ações que o app já sabe executar; o que mora fora dele, como a configuração de um runner de CI,
//! entra como passo para a pessoa fazer.
//!
//! Medido numa máquina em 10/11/2026: o SQL Server de um projeto pesava antes de 26 de 30 travas e
//! sobrecargas, e jobs de CI rodando dois de cada vez, sem limite, apareciam em várias delas.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::armazenamento::{LimpezaDocker, ResumoDocker};
use crate::docker::Conteiner;
use crate::ocorrencias::Ocorrencia;

/// As ocorrências que contam para o plano.
pub const JANELA_MS: u64 = 7 * 24 * 60 * 60 * 1000;
/// Um container entra na sugestão de limite quando pesou antes de pelo menos tantas ocorrências.
const MINIMO_DE_OCORRENCIAS: usize = 2;
/// Abaixo disso, o container não pesou de verdade em nenhuma.
const NUCLEOS_QUE_PESAM: f32 = 1.0;
/// Uma sobrecarga que começou até tanto antes da trava conta como aviso dela.
const AVISO_ANTES_MS: u64 = 10 * 60 * 1000;

#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Risco {
    /// Nada que a pessoa use se perde, e o que sai volta sozinho quando for preciso.
    Seguro,
    /// Reversível, mas muda como algo roda.
    MudaComportamento,
    /// O que sai não volta.
    Irreversivel,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Acao {
    LimparDocker { qual: LimpezaDocker, volumes: Vec<String> },
    LimitarCpu { id: String, nome: String, nucleos: f32 },
    /// O app não executa: mostra os passos.
    VoceFaz { passos: Vec<String> },
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub titulo: String,
    /// Por que o item está no plano, com o número que o justifica.
    pub motivo: String,
    /// O que muda se a pessoa fizer.
    pub efeito: String,
    pub risco: Risco,
    pub marcado: bool,
    /// Lista que a pessoa confere antes, como os volumes que serão apagados.
    pub detalhes: Vec<String>,
    pub acao: Acao,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Pesou {
    /// "sqlservr (sigc-mssql)" ou "cc1 (container que já não existe)".
    pub nome: String,
    pub ocorrencias: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct Plano {
    pub desde_ms: u64,
    pub travas: usize,
    pub sobrecargas: usize,
    /// Quem pesou antes de mais travas e sobrecargas, do mais frequente para o menos.
    pub quem_pesou: Vec<Pesou>,
    pub itens: Vec<Item>,
    /// Por que parte do plano ficou de fora, como o Docker desligado.
    pub avisos: Vec<String>,
}

/// O que o plano lê do estado do app.
pub struct Entrada<'a> {
    pub ocorrencias: &'a [Ocorrencia],
    /// Vazio com o Docker desligado ou sem leitura.
    pub docker: Option<&'a ResumoDocker>,
    pub conteineres: &'a [Conteiner],
    pub cpus_vm: Option<u32>,
    pub agora_ms: u64,
}

fn gb(bytes: u64) -> String {
    crate::recursos::gb(bytes)
}

fn anonimo(volume: &str) -> bool {
    volume.len() == 64 && volume.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Cache que o GitLab Runner guarda entre jobs: apagar deixa o próximo job mais lento.
fn cache_de_runner(volume: &str) -> bool {
    volume.starts_with("runner-") && volume.contains("-cache-")
}

/// Container de job de CI: nasce e morre a cada job, então limite posto nele não dura.
fn job_de_ci(nome: &str) -> bool {
    nome.starts_with("runner-")
}

pub fn montar(e: &Entrada) -> Plano {
    let desde_ms = e.agora_ms.saturating_sub(JANELA_MS);
    let recentes: Vec<&Ocorrencia> = e.ocorrencias.iter().filter(|o| o.inicio_ms >= desde_ms).collect();
    let pesadas: Vec<&Ocorrencia> = recentes.iter().copied().filter(|o| o.tipo == "trava" || o.tipo == "sobrecarga").collect();
    let travas: Vec<&Ocorrencia> = pesadas.iter().copied().filter(|o| o.tipo == "trava").collect();
    let sobrecargas = pesadas.len() - travas.len();

    // Por ocorrência, cada processo conta uma vez; o container entra pelo id e vira nome quando
    // ainda existe.
    let nome_do = |id: &str| e.conteineres.iter().find(|c| c.id == id).map(|c| c.nome.clone());
    let mut vezes: BTreeMap<String, usize> = BTreeMap::new();
    let mut por_conteiner: BTreeMap<String, (usize, f32)> = BTreeMap::new();
    let mut runner_visto = false;
    for o in &pesadas {
        let mut nomes = BTreeSet::new();
        let mut conteineres: BTreeMap<String, f32> = BTreeMap::new();
        for p in o.antes.iter().flat_map(|c| &c.pesados) {
            runner_visto |= p.nome.starts_with("gitlab-runner");
            let onde = match p.conteiner.as_deref() {
                Some(id) => match nome_do(id) {
                    Some(n) => {
                        let pico = conteineres.entry(id.to_string()).or_default();
                        *pico = pico.max(p.cpu_nucleos);
                        n
                    }
                    None => "container que já não existe".to_string(),
                },
                None => p.distro.clone(),
            };
            nomes.insert(format!("{} ({onde})", p.nome));
        }
        for n in nomes {
            *vezes.entry(n).or_default() += 1;
        }
        for (id, pico) in conteineres {
            let v = por_conteiner.entry(id).or_default();
            v.0 += 1;
            v.1 = v.1.max(pico);
        }
    }
    let mut quem_pesou: Vec<Pesou> = vezes.into_iter().map(|(nome, ocorrencias)| Pesou { nome, ocorrencias }).collect();
    quem_pesou.sort_by(|a, b| b.ocorrencias.cmp(&a.ocorrencias).then_with(|| a.nome.cmp(&b.nome)));
    quem_pesou.truncate(6);

    let mut itens = Vec::new();
    let mut avisos = Vec::new();

    match e.docker {
        Some(d) => itens.extend(itens_de_limpeza(d)),
        None => avisos.push("O Docker não estava ligado ou não respondeu: a limpeza dele ficou fora do plano.".to_string()),
    }

    // Limite de CPU: só em container que existe, roda sem limite e pesou de verdade em mais de uma.
    match e.cpus_vm {
        Some(cpus) if cpus >= 2 => {
            let nucleos = (cpus / 2) as f32;
            for c in e.conteineres.iter().filter(|c| c.estado == "running" && c.limite_cpus == 0.0 && !job_de_ci(&c.nome)) {
                let Some(&(n, pico)) = por_conteiner.get(&c.id) else { continue };
                if n < MINIMO_DE_OCORRENCIAS || pico < NUCLEOS_QUE_PESAM {
                    continue;
                }
                itens.push(Item {
                    id: format!("cpu:{}", c.id),
                    titulo: format!("Limitar {} a {} núcleos", c.nome, nucleos as u32),
                    motivo: format!(
                        "Ele pesava antes de {n} das {} travas e sobrecargas dos últimos 7 dias, com até {} núcleos, e hoje não tem limite de CPU.",
                        pesadas.len(),
                        crate::recursos::gb_texto(pico)
                    ),
                    efeito: format!(
                        "Fica com no máximo metade dos {cpus} núcleos da VM, e o resto sobra para as outras distros. O limite vale até o container ser recriado; para fixar, ponha cpus: {} no docker-compose dele.",
                        nucleos as u32
                    ),
                    risco: Risco::MudaComportamento,
                    marcado: false,
                    detalhes: Vec::new(),
                    acao: Acao::LimitarCpu { id: c.id.clone(), nome: c.nome.clone(), nucleos },
                });
            }
        }
        _ => avisos.push("Ainda não há leitura da VM para saber quantos núcleos ela tem: a sugestão de limite de CPU ficou fora.".to_string()),
    }

    let runner_rodando = e.conteineres.iter().any(|c| c.estado == "running" && c.nome.contains("gitlab-runner"));
    let jobs_pesaram = quem_pesou.iter().any(|p| p.nome.contains("container que já não existe"));
    if runner_rodando && (runner_visto || jobs_pesaram) {
        itens.push(Item {
            id: "runner".to_string(),
            titulo: "Limitar o GitLab Runner desta máquina".to_string(),
            motivo: "Há um GitLab Runner rodando aqui, e containers de jobs, que somem ao terminar, pesavam antes das travas. Limite posto pelo app num container de job não dura: o próximo job nasce sem ele.".to_string(),
            efeito: "Um job por vez e com teto de CPU e memória deixa folga para o resto da VM. Os jobs desta máquina ficam mais lentos; se houver outro runner no projeto, ele pega a vaga.".to_string(),
            risco: Risco::MudaComportamento,
            marcado: false,
            detalhes: Vec::new(),
            acao: Acao::VoceFaz {
                passos: vec![
                    "Abra o config.toml do runner: docker exec -it gitlab-runner vi /etc/gitlab-runner/config.toml (guarde uma cópia antes).".to_string(),
                    "Na primeira linha, deixe concurrent = 1.".to_string(),
                    "Logo abaixo de [runners.docker], acrescente cpus = \"4\" e memory = \"4g\". Ajuste se algum job precisar de mais memória.".to_string(),
                    "Salve: o runner relê o arquivo sozinho, e o próximo job já nasce com o limite.".to_string(),
                ],
            },
        });
    }

    // A sobrecarga como aviso: conta as travas que tiveram uma pouco antes.
    let avisadas = travas
        .iter()
        .filter(|t| recentes.iter().any(|s| s.tipo == "sobrecarga" && s.inicio_ms <= t.inicio_ms && t.inicio_ms - s.inicio_ms <= AVISO_ANTES_MS))
        .count();
    if avisadas > 0 {
        itens.push(Item {
            id: "aviso".to_string(),
            titulo: "Agir no aviso de VM sobrecarregada".to_string(),
            motivo: format!("Em {avisadas} das {} travas, o aviso de VM sobrecarregada apareceu nos 10 min antes.", travas.len()),
            efeito: "Pausar o container que mais pesa, ou baixar a prioridade dele, no próprio aviso, costuma vir antes de a trava chegar. Depois que o WSL para de abrir sessões, pode ser tarde.".to_string(),
            risco: Risco::Seguro,
            marcado: false,
            detalhes: Vec::new(),
            acao: Acao::VoceFaz {
                passos: vec!["Quando o aviso aparecer, use \"Pausar o container\" ou \"Baixar prioridade\" no item que mais pesa, e \"Retomar\" quando a carga baixar.".to_string()],
            },
        });
    }

    Plano { desde_ms, travas: travas.len(), sobrecargas, quem_pesou, itens, avisos }
}

fn itens_de_limpeza(d: &ResumoDocker) -> Vec<Item> {
    let recuperavel = |tipo: &str| d.categorias.iter().find(|c| c.tipo == tipo).map_or(0, |c| c.recuperavel_bytes);
    let mut itens = Vec::new();
    let limpar = |qual, volumes| Acao::LimparDocker { qual, volumes };

    if d.sobras > 0 {
        itens.push(Item {
            id: "sobras".to_string(),
            titulo: "Apagar containers parados de builds e jobs".to_string(),
            motivo: format!("{} container(s) parado(s) fora de projetos do Compose, com {} GB.", d.sobras, gb(d.sobras_bytes)),
            efeito: format!("Libera cerca de {} GB. São sobras de build e de jobs de CI que já terminaram.", gb(d.sobras_bytes)),
            risco: Risco::Seguro,
            marcado: true,
            detalhes: Vec::new(),
            acao: limpar(LimpezaDocker::Sobras, Vec::new()),
        });
    }

    let cache = recuperavel("Build Cache");
    if cache > 0 {
        itens.push(Item {
            id: "cache".to_string(),
            titulo: "Apagar o cache de build".to_string(),
            motivo: format!("O cache de build do Docker tem {} GB que nenhuma imagem usa.", gb(cache)),
            efeito: "Libera o espaço. O próximo build refaz as camadas que precisar, e por isso demora mais uma vez.".to_string(),
            risco: Risco::Seguro,
            marcado: true,
            detalhes: Vec::new(),
            acao: limpar(LimpezaDocker::CacheDeBuild, Vec::new()),
        });
    }

    let (anonimos, nomeados): (Vec<_>, Vec<_>) = d.volumes_sem_uso.iter().filter(|v| !cache_de_runner(&v.nome)).partition(|v| anonimo(&v.nome));
    let caches_de_runner = d.volumes_sem_uso.iter().filter(|v| cache_de_runner(&v.nome)).count();
    if !anonimos.is_empty() {
        let bytes: u64 = anonimos.iter().map(|v| v.bytes).sum();
        itens.push(Item {
            id: "volumes-anonimos".to_string(),
            titulo: "Apagar volumes sem nome que nenhum container usa".to_string(),
            motivo: format!("{} volume(s) sem nome e sem container, com {} GB. Em geral sobram de containers que já foram apagados.", anonimos.len(), gb(bytes)),
            efeito: format!("Libera cerca de {} GB. O que estava dentro deles não volta.", gb(bytes)),
            risco: Risco::Irreversivel,
            marcado: false,
            detalhes: Vec::new(),
            acao: limpar(LimpezaDocker::Volumes, anonimos.iter().map(|v| v.nome.clone()).collect()),
        });
    }
    if !nomeados.is_empty() {
        let bytes: u64 = nomeados.iter().map(|v| v.bytes).sum();
        let fora = if caches_de_runner > 0 { format!(" Os {caches_de_runner} cache(s) do GitLab Runner ficaram de fora: apagá-los deixa os jobs mais lentos.") } else { String::new() };
        itens.push(Item {
            id: "volumes-nomeados".to_string(),
            titulo: "Apagar volumes com nome que nenhum container usa".to_string(),
            motivo: format!("{} volume(s) com nome e sem container, com {} GB.{fora}", nomeados.len(), gb(bytes)),
            efeito: "Libera o espaço, mas o que estava dentro não volta: confira a lista. Volume sem container pode ser um banco ou uma cópia de trabalho que você ainda vai usar.".to_string(),
            risco: Risco::Irreversivel,
            marcado: false,
            detalhes: nomeados.iter().map(|v| format!("{} ({} GB)", v.nome, gb(v.bytes))).collect(),
            acao: limpar(LimpezaDocker::Volumes, nomeados.iter().map(|v| v.nome.clone()).collect()),
        });
    }

    let imagens = recuperavel("Images");
    if imagens > 0 {
        itens.push(Item {
            id: "imagens".to_string(),
            titulo: "Apagar imagens que nenhum container usa".to_string(),
            motivo: format!("O Docker conta {} GB em imagens sem container.", gb(imagens)),
            efeito: "Libera o espaço. Quem precisar de uma delas baixa de novo, o que exige rede; imagem de registro com login, ou usada só como serviço de CI, pode não voltar sozinha.".to_string(),
            risco: Risco::Irreversivel,
            marcado: false,
            detalhes: Vec::new(),
            acao: limpar(LimpezaDocker::ImagensSemUso, Vec::new()),
        });
    }
    itens
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::armazenamento::{Categoria, Volume};
    use crate::ocorrencias::Carga;
    use crate::pressao::Pesado;

    const AGORA: u64 = 100 * 24 * 60 * 60 * 1000;
    const MIN: u64 = 60 * 1000;
    const BANCO: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SUMIU: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn pesado(nome: &str, conteiner: Option<&str>, cpu: f32) -> Pesado {
        Pesado {
            distro: "docker-desktop".into(),
            nome: nome.into(),
            conteiner: conteiner.map(str::to_string),
            pids: vec![],
            quantos: 1,
            cpu_nucleos: cpu,
            rss: 0,
            esperando: 0,
            disco_bps: 0,
        }
    }

    fn ocorrencia(tipo: &str, inicio_ms: u64, pesados: Vec<Pesado>) -> Ocorrencia {
        Ocorrencia {
            tipo: tipo.into(),
            titulo: String::new(),
            inicio_ms,
            fim_ms: Some(inicio_ms + MIN),
            saida: None,
            pico_disco: 0.0,
            pico_cpu: 0.0,
            antes: vec![Carga { pesados, ..Default::default() }],
        }
    }

    fn conteiner(id: &str, nome: &str, limite_cpus: f32) -> Conteiner {
        Conteiner {
            id: id.into(),
            nome: nome.into(),
            projeto: None,
            estado: "running".into(),
            memoria_bytes: 0,
            limite_bytes: 0,
            cpu_nucleos: 0.0,
            limite_cpus,
            reinicios: 0,
            em_ciclo: false,
            saude: None,
        }
    }

    fn entrada<'a>(ocorrencias: &'a [Ocorrencia], docker: Option<&'a ResumoDocker>, conteineres: &'a [Conteiner]) -> Entrada<'a> {
        Entrada { ocorrencias, docker, conteineres, cpus_vm: Some(10), agora_ms: AGORA }
    }

    #[test]
    fn conta_quem_pesou_uma_vez_por_ocorrencia_e_nomeia_o_container() {
        let o = [
            ocorrencia("trava", AGORA - 60 * MIN, vec![pesado("sqlservr", Some(BANCO), 1.5), pesado("sqlservr", Some(BANCO), 1.7)]),
            ocorrencia("sobrecarga", AGORA - 30 * MIN, vec![pesado("sqlservr", Some(BANCO), 1.2), pesado("cc1", Some(SUMIU), 2.0)]),
            ocorrencia("devolucao", AGORA - 20 * MIN, vec![pesado("sqlservr", Some(BANCO), 9.0)]),
            ocorrencia("trava", AGORA - 8 * 24 * 60 * MIN, vec![pesado("velho", None, 9.0)]),
        ];
        let c = [conteiner(BANCO, "sigc-mssql", 0.0)];
        let p = montar(&entrada(&o, None, &c));
        assert_eq!((p.travas, p.sobrecargas), (1, 1));
        assert_eq!(p.quem_pesou[0], Pesou { nome: "sqlservr (sigc-mssql)".into(), ocorrencias: 2 });
        assert_eq!(p.quem_pesou[1], Pesou { nome: "cc1 (container que já não existe)".into(), ocorrencias: 1 });
        assert!(p.avisos.iter().any(|a| a.contains("Docker")));
    }

    #[test]
    fn sugere_limite_so_a_quem_pesou_de_verdade_mais_de_uma_vez_e_nao_tem_limite() {
        let leve = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
        let limitado = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
        let job = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
        let todos = |cpu| vec![pesado("sqlservr", Some(BANCO), cpu), pesado("php", Some(leve), 0.3), pesado("x", Some(limitado), 3.0), pesado("php", Some(job), 3.0)];
        let o = [ocorrencia("trava", AGORA - 60 * MIN, todos(1.7)), ocorrencia("sobrecarga", AGORA - 30 * MIN, todos(0.4))];
        let c = [conteiner(BANCO, "sigc-mssql", 0.0), conteiner(leve, "leve", 0.0), conteiner(limitado, "limitado", 2.0), conteiner(job, "runner-x-build", 0.0)];
        let p = montar(&entrada(&o, None, &c));
        let limites: Vec<&Item> = p.itens.iter().filter(|i| matches!(i.acao, Acao::LimitarCpu { .. })).collect();
        assert_eq!(limites.len(), 1);
        assert_eq!(limites[0].acao, Acao::LimitarCpu { id: BANCO.into(), nome: "sigc-mssql".into(), nucleos: 5.0 });
        assert!(!limites[0].marcado);
        assert!(limites[0].motivo.contains("2 das 2"));
    }

    #[test]
    fn limpeza_marca_so_o_seguro_e_separa_volumes_sem_tocar_no_cache_do_runner() {
        let anon = "f".repeat(64);
        let d = ResumoDocker {
            categorias: vec![
                Categoria { tipo: "Build Cache".into(), recuperavel_bytes: 2 << 30, ..Default::default() },
                Categoria { tipo: "Images".into(), recuperavel_bytes: 10 << 30, ..Default::default() },
            ],
            volumes_sem_uso: vec![
                Volume { nome: anon.clone(), bytes: 1 << 20 },
                Volume { nome: "sigc_src_velho".into(), bytes: 200 << 20 },
                Volume { nome: "runner-abc-cache-def".into(), bytes: 800 << 20 },
            ],
            sobras: 3,
            sobras_bytes: 7 << 30,
            projetos: vec![],
        };
        let p = montar(&entrada(&[], Some(&d), &[]));
        let por_id = |id: &str| p.itens.iter().find(|i| i.id == id).unwrap();
        assert!(por_id("sobras").marcado && por_id("cache").marcado);
        assert!(!por_id("imagens").marcado && !por_id("volumes-anonimos").marcado && !por_id("volumes-nomeados").marcado);
        assert_eq!(por_id("volumes-anonimos").acao, Acao::LimparDocker { qual: LimpezaDocker::Volumes, volumes: vec![anon] });
        assert_eq!(por_id("volumes-nomeados").acao, Acao::LimparDocker { qual: LimpezaDocker::Volumes, volumes: vec!["sigc_src_velho".into()] });
        assert!(por_id("volumes-nomeados").motivo.contains("1 cache(s) do GitLab Runner ficaram de fora"));
        assert!(p.itens.iter().all(|i| i.risco != Risco::Seguro || i.marcado || matches!(i.acao, Acao::VoceFaz { .. })));
    }

    #[test]
    fn runner_e_aviso_entram_so_com_evidencia() {
        let runner = "9".repeat(64);
        let c = [conteiner(&runner, "gitlab-runner", 0.0)];
        let o = [
            ocorrencia("sobrecarga", AGORA - 70 * MIN, vec![pesado("cc1", Some(SUMIU), 2.0)]),
            ocorrencia("trava", AGORA - 65 * MIN, vec![]),
            ocorrencia("trava", AGORA - 10 * MIN, vec![]),
        ];
        let p = montar(&entrada(&o, None, &c));
        assert!(matches!(p.itens.iter().find(|i| i.id == "runner").unwrap().acao, Acao::VoceFaz { .. }));
        assert!(p.itens.iter().find(|i| i.id == "aviso").unwrap().motivo.starts_with("Em 1 das 2 travas"));
        // Sem runner rodando, ou sem nada que indique jobs, o item não aparece.
        let sem = montar(&entrada(&o, None, &[]));
        assert!(sem.itens.iter().all(|i| i.id != "runner"));
        let calmo = montar(&entrada(&[], None, &c));
        assert!(calmo.itens.iter().all(|i| i.id != "runner" && i.id != "aviso"));
    }
}
