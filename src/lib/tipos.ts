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

export type TipoEvento = 'distro' | 'processo' | 'memoria' | 'config' | 'wsl';

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

/** Pedido de confirmação para ação que encerra processos em massa. */
export interface Pedido {
  titulo: string;
  texto: string;
  acao: string;
  executar: () => void;
}

export type Pagina = 'processos' | 'desempenho' | 'portas' | 'atividade' | 'distros' | 'log';

export const SIGTERM = 15;
export const SIGKILL = 9;

export interface Atualizacao {
  versao: string;
  notas: string | null;
  /** Falso na versão portátil, que não se substitui sozinha. */
  instalavel: boolean;
}
