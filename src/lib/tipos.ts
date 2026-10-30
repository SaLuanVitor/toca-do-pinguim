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
  /** As três partes do que o kernel descarta quando precisa. */
  mem_blocos: number;
  mem_arquivos: number;
  mem_tabelas: number;
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

export type TipoEvento = 'distro' | 'processo' | 'memoria' | 'config' | 'wsl' | 'docker' | 'armazenamento';

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
  /** 0 quando o container não tem limite de memória. */
  limite_bytes: number;
  reinicios: number;
  em_ciclo: boolean;
  /** healthy, unhealthy ou starting; null sem healthcheck. */
  saude: string | null;
}

/** Processo com sinal de estar preso, vindo do vigia do backend. */
export interface Suspeito {
  pid: number;
  ppid: number;
  nome: string;
  motivo: 'aguardando_es' | 'zumbi' | 'cpu_constante';
  desde_ms: number;
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

/** Devolução do cache da VM e o efeito medido na RAM do Windows. */
export interface Devolucao {
  automatica: boolean;
  quando_ms: number;
  antes_bytes: number;
  logo_bytes: number | null;
  depois_bytes: number | null;
  concluida: boolean;
}

export interface EstadoRecursos {
  memoria: MemoriaWindows;
  docker: Docker;
  saude: SituacaoWsl | null;
  devolucao: Devolucao | null;
  devolver_cache_automatico: boolean;
  gatilho: Gatilho;
}

/** Quando a devolução automática do cache dispara; basta um dos dois. */
export interface Gatilho {
  minimo_livre_pct: number;
  /** null não olha o tamanho do cache. */
  cache_maximo_bytes: number | null;
}

export interface DiscoVirtual {
  nome: string;
  caminho: string;
  tamanho_bytes: number;
  /** Só com a distro ligada. */
  usado_bytes: number | null;
  esparso: boolean;
  nota: string | null;
  /** Falso no esparso, no que ficou para trás e no menor que 1 GB. */
  compactavel: boolean;
}

export interface Compactacao {
  antes_bytes: number;
  depois_bytes: number;
}

export interface CategoriaDocker {
  /** Images, Containers, Local Volumes ou Build Cache. */
  tipo: string;
  total_bytes: number;
  recuperavel_bytes: number;
  quantidade: number;
  ativos: number;
}

export interface ResumoDocker {
  categorias: CategoriaDocker[];
  volumes_sem_uso: { nome: string; bytes: number }[];
  sobras: number;
  sobras_bytes: number;
  /** Do que mais ocupa para o que menos ocupa. */
  projetos: ProjetoDocker[];
}

export interface VolumeProjeto {
  nome: string;
  bytes: number;
  /** Algum container, ligado ou parado, ainda o usa. */
  em_uso: boolean;
}

/** O espaço em disco de um projeto do Docker Compose. */
export interface ProjetoDocker {
  nome: string;
  containers: number;
  parados: number;
  containers_bytes: number;
  /** Só a parte das imagens que nenhum outro projeto usa. */
  imagens_bytes: number;
  volumes: VolumeProjeto[];
}

export interface LimpezaProgramada {
  cache_de_build: boolean;
  sobras_de_containers: boolean;
  tmp_dias: number | null;
  ultima_ms: number | null;
}

export interface Armazenamento {
  discos: DiscoVirtual[];
  docker: ResumoDocker | null;
  erro_docker: string | null;
  limpeza: LimpezaProgramada;
  distros_ligadas: string[];
  /** Todas as registradas, ligadas ou não. */
  distros: string[];
}

export interface Pasta {
  caminho: string;
  bytes: number;
  modificado_ms: number | null;
  em_uso_por: string | null;
  apagavel: boolean;
}

export interface AnalisePastas {
  distro: string;
  raiz: Pasta[];
  tmp: Pasta[];
  caches: Pasta[];
}

export type LimpezaDocker = 'sobras' | 'parados' | 'imagens_sem_uso' | 'cache_de_build' | 'volumes';

/** Pedido de confirmação para ação que encerra processos em massa. */
export interface Pedido {
  titulo: string;
  texto: string;
  acao: string;
  executar: () => void;
}

export type AbaDesempenho = 'processamento' | 'memoria' | 'disco';

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

/** O que a máquina comporta, para limitar os campos de configuração. */
export interface Limites {
  ram_bytes: number;
  processadores: number;
  discos: { letra: string; livre_bytes: number }[];
}

export interface AtalhoTeclado {
  /** No formato do Windows, como Alt+Ctrl+P; vazio sem tecla. */
  tecla: string;
  caminho: string;
  existe: boolean;
}
