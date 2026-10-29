// Espelho de crates/protocolo e de src-tauri/src/wsl.rs.

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
  agente: { rss: number; cpu_pct: number; coleta_us: number };
}

export interface Distro {
  nome: string;
  versao: number;
  padrao: boolean;
  rodando: boolean;
}

/** Pedido de confirmação para ação que encerra processos em massa. */
export interface Pedido {
  titulo: string;
  texto: string;
  acao: string;
  executar: () => void;
}

export const SIGTERM = 15;
export const SIGKILL = 9;
