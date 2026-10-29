import type { Amostra, Distro, Evento, Pagina, Pedido, Porta } from './tipos';

const PONTOS_HISTORICO = 60;
const EVENTOS_NA_TELA = 500;

export interface Ponto {
  cpu: number;
  mem: number;
  swap: number;
}

export interface Aviso {
  id: number;
  texto: string;
}

/** Porta já resolvida entre as distros: a rede é uma só, o dono aparece em uma delas. */
export interface PortaVista extends Porta {
  distro: string | null;
  nome: string | null;
}

class Estado {
  // `raw`: cada amostra substitui a anterior inteira, e centenas de processos por
  // segundo não precisam de proxy reativo campo a campo.
  distros = $state.raw<Distro[]>([]);
  amostras = $state.raw<Record<string, Amostra>>({});
  portasPorDistro = $state.raw<Record<string, Porta[]>>({});
  historico = $state.raw<Ponto[]>([]);
  eventos = $state.raw<Evento[]>([]);
  /** Distros cujo agente saiu por ociosidade e que o WSL deve desligar em seguida. */
  ociosas = $state.raw<Set<string>>(new Set());
  avisos = $state<Aviso[]>([]);
  intervalo = $state(1000);
  demonstracao = $state(false);
  pagina = $state<Pagina>('processos');
  pedido = $state<Pedido | null>(null);
  paletaAberta = $state(false);
  /** Processo a selecionar na aba Processos, pedido de fora dela (pela busca). */
  foco = $state<{ distro: string; pid: number } | null>(null);
  /** Filtro da aba Portas e abertura do ajuste de limite, pedidos pela busca. */
  buscaPortas = $state('');
  ajustarLimite = $state(false);
  #proximoAviso = 1;

  /** Todas as distros dividem a mesma VM; a primeira em ordem alfabética fala por ela. */
  get fonteVm(): string | undefined {
    return Object.keys(this.amostras).sort()[0];
  }

  get vm(): Amostra | undefined {
    const fonte = this.fonteVm;
    return fonte ? this.amostras[fonte] : undefined;
  }

  get portas(): PortaVista[] {
    const unicas = new Map<string, PortaVista>();
    for (const [distro, lista] of Object.entries(this.portasPorDistro)) {
      for (const p of lista) {
        const chave = `${p.protocolo}|${p.endereco}|${p.porta}`;
        if (unicas.has(chave) && p.pid === null) continue;
        const nome = p.pid === null ? null : (this.amostras[distro]?.processos.find((x) => x.pid === p.pid)?.nome ?? null);
        unicas.set(chave, { ...p, distro: p.pid === null ? null : distro, nome });
      }
    }
    return [...unicas.values()];
  }

  receberDistros(lista: Distro[]) {
    this.distros = lista;
    const rodando = new Set(lista.filter((d) => d.rodando).map((d) => d.nome));
    const manter = <T>(r: Record<string, T>) => Object.fromEntries(Object.entries(r).filter(([n]) => rodando.has(n)));
    if (Object.keys(this.amostras).some((n) => !rodando.has(n))) this.amostras = manter(this.amostras);
    if (Object.keys(this.portasPorDistro).some((n) => !rodando.has(n))) this.portasPorDistro = manter(this.portasPorDistro);
    if ([...this.ociosas].some((n) => !rodando.has(n))) this.ociosas = new Set([...this.ociosas].filter((n) => rodando.has(n)));
  }

  receberAmostra(distro: string, a: Amostra) {
    this.amostras = { ...this.amostras, [distro]: a };
    if (a.portas) this.portasPorDistro = { ...this.portasPorDistro, [distro]: a.portas };
    if (this.ociosas.has(distro)) this.ociosas = new Set([...this.ociosas].filter((n) => n !== distro));
    if (distro !== this.fonteVm) return;
    const mem = a.mem_total ? (1 - a.mem_disponivel / a.mem_total) * 100 : 0;
    const swap = a.swap_total ? (1 - a.swap_livre / a.swap_total) * 100 : 0;
    this.historico = [...this.historico, { cpu: a.cpu_vm_pct, mem, swap }].slice(-PONTOS_HISTORICO);
  }

  agenteParou(distro: string) {
    if (distro in this.amostras) {
      const { [distro]: _, ...resto } = this.amostras;
      this.amostras = resto;
    }
    if (distro in this.portasPorDistro) {
      const { [distro]: _, ...resto } = this.portasPorDistro;
      this.portasPorDistro = resto;
    }
  }

  distroOciosa(distro: string) {
    this.ociosas = new Set([...this.ociosas, distro]);
  }

  receberEventos(lista: Evento[]) {
    this.eventos = lista.slice(-EVENTOS_NA_TELA);
  }

  receberEvento(e: Evento) {
    this.eventos = [...this.eventos, e].slice(-EVENTOS_NA_TELA);
  }

  avisar(texto: string) {
    const id = this.#proximoAviso++;
    this.avisos.push({ id, texto });
    setTimeout(() => this.dispensar(id), 8000);
  }

  dispensar(id: number) {
    this.avisos = this.avisos.filter((a) => a.id !== id);
  }
}

export const estado = new Estado();
