// Ordenação comum a todas as tabelas: comparação natural em pt-BR, vazios sempre no fim
// e a escolha de coluna lembrada entre trocas de aba e aberturas do app.

import { lerPreferencia, salvarPreferencia } from './preferencias';

const colacao = new Intl.Collator('pt-BR', { sensitivity: 'base', numeric: true });

function vazio(v: unknown): boolean {
  return v === null || v === undefined || v === '';
}

/** Números por valor, texto em ordem natural ("Relay(9)" antes de "Relay(10)"). */
export function comparar(a: unknown, b: unknown): number {
  if (typeof a === 'number' && typeof b === 'number') return a - b;
  return colacao.compare(String(a), String(b));
}

export class Ordenacao<C extends string> {
  coluna = $state<C>() as C;
  crescente = $state(true);
  #chave: string;
  #crescentePadrao: (c: C) => boolean;

  /**
   * `crescentePadrao` diz a direção ao clicar numa coluna nova: medidas (CPU, memória)
   * começam pelo maior, como no Gerenciador de Tarefas; nomes, pelo começo do alfabeto.
   */
  constructor(chave: string, colunaPadrao: C, crescentePadrao: (c: C) => boolean) {
    this.#chave = `ordem.${chave}`;
    this.#crescentePadrao = crescentePadrao;
    const salva = lerPreferencia<{ coluna: C; crescente: boolean } | null>(this.#chave, null);
    this.coluna = salva?.coluna ?? colunaPadrao;
    this.crescente = salva?.crescente ?? crescentePadrao(colunaPadrao);
  }

  alternar(c: C) {
    if (this.coluna === c) this.crescente = !this.crescente;
    else {
      this.coluna = c;
      this.crescente = this.#crescentePadrao(c);
    }
    salvarPreferencia(this.#chave, { coluna: this.coluna, crescente: this.crescente });
  }

  ariaSort(c: C): 'ascending' | 'descending' | 'none' {
    if (this.coluna !== c) return 'none';
    return this.crescente ? 'ascending' : 'descending';
  }

  #fixos = new Map<string, number>();
  #assinaturaFixos = '';

  /**
   * Ordena por um valor fixo de cada item, que só acompanha o valor real quando ele se
   * afasta mais que `tolerancia`. A lista sai ordenada exatamente pelos valores fixos, e
   * cada um está a no máximo uma tolerância do real: dois itens quaisquer nunca aparecem
   * invertidos por mais que duas tolerâncias, em qualquer distância da lista. Valor que
   * oscila dentro da faixa não mexe a linha.
   *
   * `assinatura` diferente da anterior (outra coluna, direção ou filtro) descarta os
   * valores fixos e ordena pelos reais.
   */
  ordenarEstavel<T>(
    lista: readonly T[],
    valor: (t: T, c: C) => unknown,
    chave: (t: T) => string,
    tolerancia: (v: number) => number,
    desempate: (a: T, b: T) => number,
    assinatura: string,
  ): T[] {
    if (assinatura !== this.#assinaturaFixos || this.#fixos.size > 5000) {
      this.#fixos.clear();
      this.#assinaturaFixos = assinatura;
    }
    const c = this.coluna;
    const pares = lista.map((t) => {
      const v = valor(t, c);
      if (typeof v !== 'number') return { t, v };
      const k = chave(t);
      const f = this.#fixos.get(k);
      if (f !== undefined && Math.abs(v - f) <= tolerancia(v)) return { t, v: f };
      this.#fixos.set(k, v);
      return { t, v };
    });
    return this.ordenar(pares, (p) => p.v, (a, b) => desempate(a.t, b.t)).map((p) => p.t);
  }

  /**
   * Devolve uma cópia ordenada. Vazio fica no fim nas duas direções. Empate na coluna
   * cai em `desempate`, que não inverte com a direção: assim duas linhas de mesmo valor
   * mostrado não trocam de lugar a cada atualização.
   */
  ordenar<T>(lista: readonly T[], valor: (t: T, c: C) => unknown, desempate?: (a: T, b: T) => number): T[] {
    const c = this.coluna;
    const sinal = this.crescente ? 1 : -1;
    return [...lista].sort((a, b) => {
      const va = valor(a, c);
      const vb = valor(b, c);
      if (vazio(va) || vazio(vb)) {
        if (vazio(va) && vazio(vb)) return desempate?.(a, b) ?? 0;
        return vazio(va) ? 1 : -1;
      }
      return sinal * comparar(va, vb) || (desempate?.(a, b) ?? 0);
    });
  }
}
