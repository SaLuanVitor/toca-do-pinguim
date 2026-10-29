import type { Amostra, Distro } from './tipos';

const PONTOS_HISTORICO = 60;

export interface Ponto {
  cpu: number;
  mem: number;
  swap: number;
}

export interface Aviso {
  id: number;
  texto: string;
}

class Estado {
  // `raw`: cada amostra substitui a anterior inteira, e centenas de processos por
  // segundo não precisam de proxy reativo campo a campo.
  distros = $state.raw<Distro[]>([]);
  amostras = $state.raw<Record<string, Amostra>>({});
  historico = $state.raw<Ponto[]>([]);
  avisos = $state<Aviso[]>([]);
  intervalo = $state(1000);
  demonstracao = $state(false);
  #proximoAviso = 1;

  /** Todas as distros dividem a mesma VM; a primeira em ordem alfabética fala por ela. */
  get fonteVm(): string | undefined {
    return Object.keys(this.amostras).sort()[0];
  }

  get vm(): Amostra | undefined {
    const fonte = this.fonteVm;
    return fonte ? this.amostras[fonte] : undefined;
  }

  receberDistros(lista: Distro[]) {
    this.distros = lista;
    const rodando = new Set(lista.filter((d) => d.rodando).map((d) => d.nome));
    const mantidas = Object.entries(this.amostras).filter(([nome]) => rodando.has(nome));
    if (mantidas.length !== Object.keys(this.amostras).length) {
      this.amostras = Object.fromEntries(mantidas);
    }
  }

  receberAmostra(distro: string, a: Amostra) {
    this.amostras = { ...this.amostras, [distro]: a };
    if (distro !== this.fonteVm) return;
    const mem = a.mem_total ? (1 - a.mem_disponivel / a.mem_total) * 100 : 0;
    const swap = a.swap_total ? (1 - a.swap_livre / a.swap_total) * 100 : 0;
    this.historico = [...this.historico, { cpu: a.cpu_vm_pct, mem, swap }].slice(-PONTOS_HISTORICO);
  }

  agenteParou(distro: string) {
    if (!(distro in this.amostras)) return;
    const { [distro]: _, ...resto } = this.amostras;
    this.amostras = resto;
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
