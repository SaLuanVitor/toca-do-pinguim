<script lang="ts">
  import { estado } from './estado.svelte';
  import type { Evento, TipoEvento } from './tipos';

  const FILTROS: { id: TipoEvento | 'todos'; titulo: string }[] = [
    { id: 'todos', titulo: 'Tudo' },
    { id: 'distro', titulo: 'Distros' },
    { id: 'wsl', titulo: 'WSL' },
    { id: 'processo', titulo: 'Processos' },
    { id: 'docker', titulo: 'Docker' },
    { id: 'armazenamento', titulo: 'Disco' },
    { id: 'memoria', titulo: 'Falta de RAM' },
    { id: 'config', titulo: 'Configuração' },
  ];

  const ROTULOS: Record<TipoEvento, string> = {
    distro: 'Distro',
    processo: 'Processo',
    memoria: 'Memória',
    config: 'Configuração',
    wsl: 'WSL',
    docker: 'Docker',
    armazenamento: 'Disco',
  };

  let filtro = $state<TipoEvento | 'todos'>('todos');

  function alerta(e: Evento): boolean {
    if (e.tipo === 'memoria') return !e.texto.includes('voltou a ter folga');
    if (e.tipo === 'wsl') return e.texto.includes('deixou de responder');
    return e.tipo === 'docker' && e.texto.includes('sem parar');
  }

  const hora = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  const data = new Intl.DateTimeFormat('pt-BR', { day: '2-digit', month: '2-digit', year: 'numeric' });

  function dia(ms: number): string {
    const d = new Date(ms);
    const hoje = new Date();
    const ontem = new Date(Date.now() - 86_400_000);
    if (d.toDateString() === hoje.toDateString()) return 'Hoje';
    if (d.toDateString() === ontem.toDateString()) return 'Ontem';
    return data.format(d);
  }

  const grupos = $derived.by(() => {
    const lista = estado.eventos
      .filter((e) => filtro === 'todos' || e.tipo === filtro)
      .toReversed();
    const saida: { dia: string; eventos: Evento[] }[] = [];
    for (const e of lista) {
      const d = dia(e.quando_ms);
      if (saida.at(-1)?.dia !== d) saida.push({ dia: d, eventos: [] });
      saida.at(-1)!.eventos.push(e);
    }
    return saida;
  });
</script>

<header class="barra">
  <h1>Atividade</h1>
  <div class="filtros" role="group" aria-label="Filtrar por tipo">
    {#each FILTROS as f (f.id)}
      <button class="botao" aria-pressed={filtro === f.id} onclick={() => (filtro = f.id)}>{f.titulo}</button>
    {/each}
  </div>
</header>

<div class="rolagem">
  {#if grupos.length === 0}
    <div class="vazio">
      <strong>Nada registrado {filtro === 'todos' ? 'ainda' : 'neste filtro'}</strong>
      <span>Distros que ligam e desligam, quedas do WSL, processos encerrados pelo app, falta de memória e mudanças de configuração aparecem aqui.</span>
    </div>
  {/if}
  {#each grupos as g (g.dia)}
    <section aria-labelledby="dia-{g.dia}">
      <h2 id="dia-{g.dia}">{g.dia}</h2>
      <ol>
        {#each g.eventos as e (`${e.quando_ms}|${e.texto}`)}
          <li class:alerta={alerta(e)}>
            <time datetime={new Date(e.quando_ms).toISOString()}>{hora.format(e.quando_ms)}</time>
            <span class="tipo">{ROTULOS[e.tipo] ?? e.tipo}</span>
            <div class="texto">
              {e.texto}
              {#if e.detalhe}
                <details>
                  <summary>Linha do kernel</summary>
                  <code>{e.detalhe}</code>
                </details>
              {/if}
            </div>
          </li>
        {/each}
      </ol>
    </section>
  {/each}
</div>

<style>
  .filtros {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }

  .filtros .botao {
    height: 28px;
  }

  .filtros .botao[aria-pressed='true'] {
    background: var(--selecionado);
    border-color: var(--destaque);
    color: var(--destaque);
  }

  .rolagem {
    flex: 1;
    overflow: auto;
    padding: 0 20px 20px;
  }

  h2 {
    margin: 16px 0 6px;
    color: var(--texto-2);
    font-size: 12px;
    font-weight: 600;
  }

  ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: 72px 104px 1fr;
    gap: 12px;
    align-items: baseline;
    padding: 9px 10px;
    border-bottom: 1px solid var(--borda);
    font-size: 13px;
  }

  li.alerta {
    border-left: 3px solid var(--perigo);
  }

  time {
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }

  .tipo {
    color: var(--texto-2);
    font-size: 12px;
  }

  details {
    margin-top: 4px;
    color: var(--texto-2);
  }

  code {
    display: block;
    margin-top: 4px;
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-all;
  }

  @media (max-width: 560px) {
    li {
      grid-template-columns: 64px 1fr;
    }

    .tipo {
      display: none;
    }
  }
</style>
