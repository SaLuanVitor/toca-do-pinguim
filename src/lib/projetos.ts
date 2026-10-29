import type { Conteiner } from './tipos';

/** Projeto do Docker Compose, ou um container avulso sozinho no grupo dele. */
export interface Projeto {
  chave: string;
  nome: string;
  avulso: boolean;
  conteineres: Conteiner[];
  rodando: number;
  memoria: number;
  emCiclo: Conteiner[];
}

export function ativo(c: Conteiner): boolean {
  return c.estado === 'running' || c.estado === 'restarting';
}

/** Os que mais ocupam vêm primeiro; os parados vão para o fim. */
export function agrupar(conteineres: Conteiner[]): Projeto[] {
  const grupos = new Map<string, Projeto>();
  for (const c of conteineres) {
    const chave = c.projeto ? `p:${c.projeto}` : `c:${c.id}`;
    let g = grupos.get(chave);
    if (!g) {
      g = { chave, nome: c.projeto ?? c.nome, avulso: !c.projeto, conteineres: [], rodando: 0, memoria: 0, emCiclo: [] };
      grupos.set(chave, g);
    }
    g.conteineres.push(c);
    g.memoria += c.memoria_bytes;
    if (ativo(c)) g.rodando++;
    if (c.em_ciclo) g.emCiclo.push(c);
  }
  return [...grupos.values()].sort((a, b) => b.memoria - a.memoria || b.rodando - a.rodando || a.nome.localeCompare(b.nome));
}
