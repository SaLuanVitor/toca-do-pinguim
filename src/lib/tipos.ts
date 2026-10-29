// Espelho de crates/protocolo e dos tipos serializados por src-tauri.

export interface Processo {
  pid: number;
  ppid: number;
  usuario: string;
  estado: string;
  nome: string;
  cmdline: string;
  cpu_pct: number;
  rss: number;
}

export interface Porta {
  protocolo: string;
  endereco: string;
  porta: number;
  pid: number | null;
}

export interface Amostra {
  seq: number;
  uptime_s: number;
  cpus: number;
  cpu_vm_pct: number;
  mem_total: number;
  mem_disponivel: number;
  mem_livre: number;
  swap_total: number;
  swap_livre: number;
  processos: Processo[];
  portas?: Porta[];
  agente: { rss: number; cpu_pct: number; coleta_us: number };
}

export interface Distro {
  nome: string;
  versao: number;
  padrao: boolean;
  rodando: boolean;
}

export type TipoEvento = 'distro' | 'processo' | 'memoria' | 'config' | 'wsl' | 'docker';

export interface Evento {
  quando_ms: number;
  tipo: TipoEvento;
  distro: string | null;
  texto: string;
  detalhe?: string;
}

export interface ResumoMemoria {
  limite_bytes: number;
  limite_configurado: boolean;
  ram_host_bytes: number;
  minutos_de_dados: number;
  pico_bytes: number;
  sugestao_bytes: number | null;
}

export interface MemoriaWindows {
  total_bytes: number;
  disponivel_bytes: number;
  commit_bytes: number;
  commit_limite_bytes: number;
  /** O que a VM do WSL ocupa no Windows, cache incluso; null com o WSL desligado. */
  wsl_bytes: number | null;
  pressao: boolean;
}

export interface Conteiner {
  id: string;
  nome: string;
  projeto: string | null;
  /** Como o Docker diz: running, restarting, exited, paused. */
  estado: string;
  memoria_bytes: number;
  reinicios: number;
  em_ciclo: boolean;
}

export interface Docker {
  ligado: boolean;
  conteineres: Conteiner[];
  erro: string | null;
}

/** Queda em curso do WSL; null quando ele responde. */
export interface SituacaoWsl {
  alvo: string | null;
  motivo: string;
  desde_ms: number;
}

export interface EstadoRecursos {
  memoria: MemoriaWindows;
  docker: Docker;
  saude: SituacaoWsl | null;
}

/** Pedido de confirmação para ação que encerra processos em massa. */
export interface Pedido {
  titulo: string;
  texto: string;
  acao: string;
  executar: () => void;
}

export type Pagina = 'processos' | 'desempenho' | 'portas' | 'atividade' | 'distros' | 'configuracoes' | 'log';

export const SIGTERM = 15;
export const SIGKILL = 9;

export interface Atualizacao {
  versao: string;
  notas: string | null;
  /** Falso na versão portátil, que não se substitui sozinha. */
  instalavel: boolean;
}

export interface ConteudoWslconfig {
  caminho: string;
  existe: boolean;
  /** [seção, chave, valor], na ordem do arquivo. */
  valores: [string, string, string][];
}

export interface MudancaWslconfig {
  secao: string;
  chave: string;
  /** null tira a chave e volta ao padrão do WSL. */
  valor: string | null;
}

export interface AtalhoTeclado {
  /** No formato do Windows, como Alt+Ctrl+P; vazio sem tecla. */
  tecla: string;
  caminho: string;
  existe: boolean;
}
