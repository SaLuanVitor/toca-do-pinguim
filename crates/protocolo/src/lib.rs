//! Contrato entre o agente (dentro da distro) e o app (no Windows).
//!
//! Uma mensagem JSON por linha nos dois sentidos: o agente escreve [`Mensagem`] na saída
//! padrão e lê [`Comando`] da entrada padrão. O mesmo crate compila para os dois lados,
//! então o contrato não tem como divergir.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Mensagem {
    Amostra(Amostra),
    Resposta { ok: bool, erro: Option<String> },
    /// A sessão do agente é a única aberta na distro. O agente avisa e sai, para o WSL
    /// poder desligá-la por ociosidade como faria sem o app.
    Ocioso,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Comando {
    Sinal { pid: i32, sinal: i32 },
    /// Nice para o processo, as threads dele e os descendentes vivos.
    Prioridade { pid: i32, nice: i32 },
    Intervalo { ms: u64 },
    Sair,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Amostra {
    pub seq: u64,
    /// Uptime da VM, que é o relógio usado nas taxas: não anda quando o host suspende.
    pub uptime_s: f64,
    pub cpus: u32,
    /// CPU da VM inteira. No WSL2 todas as distros dividem o mesmo kernel, então
    /// `/proc/stat` não separa uma distro da outra.
    pub cpu_vm_pct: f32,
    pub mem_total: u64,
    pub mem_disponivel: u64,
    /// Memória sem uso nenhum. A diferença para `mem_disponivel` é cache que o kernel
    /// descarta quando precisa, mas que até lá a VM segura no Windows.
    #[serde(default)]
    pub mem_livre: u64,
    /// As três partes do que o kernel descarta quando precisa: blocos do disco
    /// (`Buffers`), conteúdo de arquivos lidos (`Cached` sem a memória compartilhada, que
    /// não é descartável) e as tabelas de arquivos percorridos (`SReclaimable`).
    #[serde(default)]
    pub mem_blocos: u64,
    #[serde(default)]
    pub mem_arquivos: u64,
    #[serde(default)]
    pub mem_tabelas: u64,
    pub swap_total: u64,
    pub swap_livre: u64,
    /// Só os processos desta distro: cada uma tem o próprio namespace de PID.
    pub processos: Vec<Processo>,
    /// Ausente nas amostras em que a varredura de portas não rodou; o app mantém a última.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portas: Option<Vec<Porta>>,
    /// Linhas do log do kernel lidas desde a amostra anterior que interessam ao usuário.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kernel: Vec<EventoKernel>,
    /// Da VM inteira, como a CPU. Ausente em kernel sem `/proc/pressure` e em agente antigo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressao: Option<Box<Pressao>>,
    pub agente: CustoAgente,
}

/// Uma linha de `/proc/pressure`: em %, a fatia do tempo em que tarefas esperaram pelo
/// recurso, nas médias de 10 s e de 60 s.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq)]
pub struct Espera {
    pub avg10: f32,
    pub avg60: f32,
}

/// Pressão do kernel. `some`: ao menos uma tarefa esperava pelo recurso. `full`: todas as que
/// não estavam ociosas esperavam ao mesmo tempo, e a VM não andava.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq)]
pub struct Pressao {
    pub cpu_some: Espera,
    pub memoria_some: Espera,
    pub memoria_full: Espera,
    pub disco_some: Espera,
    pub disco_full: Espera,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Processo {
    pub pid: i32,
    pub ppid: i32,
    pub usuario: String,
    pub estado: char,
    pub nome: String,
    pub cmdline: String,
    /// Percentual do total de CPUs, como o Gerenciador de Tarefas mostra.
    pub cpu_pct: f32,
    pub rss: u64,
    /// De -20 a 19; acima de 0, o processo cede a vez aos de prioridade normal.
    #[serde(default)]
    pub nice: i8,
    /// Leitura e gravação no disco, em bytes por segundo, desde a amostra anterior. Vem de
    /// `/proc/<pid>/io`, que só o root lê para processos de outros usuários.
    #[serde(default)]
    pub disco_bps: u64,
}

/// Socket em escuta. A rede é uma só para todas as distros, então o mesmo socket aparece
/// na amostra de cada uma; o `pid` só vem preenchido na distro dona do processo.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Porta {
    pub protocolo: String,
    pub endereco: String,
    pub porta: u16,
    pub pid: Option<i32>,
}

/// O log do kernel é um só para a VM: a mesma linha chega pelos agentes de todas as
/// distros, e o `seq` do kernel é o que permite descartar as repetidas.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EventoKernel {
    pub seq: u64,
    pub texto: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CustoAgente {
    pub rss: u64,
    pub cpu_pct: f32,
    pub coleta_us: u64,
}
