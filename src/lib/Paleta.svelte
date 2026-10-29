<script lang="ts">
  // Busca única (Ctrl+K): páginas, ações, processos e portas na mesma caixa.
  import { desligarWsl, iniciar, reiniciar, terminar } from './acoes.svelte';
  import { estado } from './estado.svelte';
  import { pct } from './formato';
  import type { Pagina } from './tipos';

  interface Item {
    id: string;
    titulo: string;
    detalhe: string;
    categoria: string;
    peso: number;
    executar: () => void;
  }

  const PAGINAS: [Pagina, string][] = [
    ['processos', 'Processos'],
    ['desempenho', 'Desempenho'],
    ['portas', 'Portas'],
    ['atividade', 'Atividade'],
    ['distros', 'Distros'],
  ];
  const MAXIMO = 12;
  const PROCESSOS_NO_MAXIMO = 8;

  let dialogo: HTMLDialogElement;
  let campo = $state<HTMLInputElement>();
  let termo = $state('');
  let ativo = $state(0);

  /** 3 igual, 2 começa com, 1 contém todas as palavras, 0 não serve. */
  function casar(texto: string, t: string): number {
    const x = texto.toLowerCase();
    if (!t) return 1;
    if (x === t) return 3;
    if (x.startsWith(t)) return 2;
    return t.split(/\s+/).every((p) => x.includes(p)) ? 1 : 0;
  }

  const itens = $derived.by((): Item[] => {
    const t = termo.trim().toLowerCase();
    const r: Item[] = [];
    const add = (i: Omit<Item, 'peso'>, peso: number) => peso && r.push({ ...i, peso });

    for (const [p, nome] of PAGINAS) {
      add({ id: `p:${p}`, titulo: `Ir para ${nome}`, detalhe: '', categoria: 'Página', executar: () => (estado.pagina = p) }, casar(`${nome} ir para`, t));
    }
    add(
      {
        id: 'a:limite',
        titulo: 'Ajustar limite de memória',
        detalhe: 'Sugestão a partir do uso medido',
        categoria: 'Ação',
        executar: () => {
          estado.pagina = 'desempenho';
          estado.ajustarLimite = true;
        },
      },
      casar('ajustar limite de memória ram wslconfig', t),
    );
    if (estado.distros.some((d) => d.rodando)) {
      add({ id: 'a:desligar', titulo: 'Desligar o WSL', detalhe: 'Termina todas as distros', categoria: 'Ação', executar: desligarWsl }, casar('desligar o wsl shutdown', t));
    }
    for (const d of estado.distros) {
      if (d.rodando) {
        add({ id: `r:${d.nome}`, titulo: `Reiniciar ${d.nome}`, detalhe: '', categoria: 'Ação', executar: () => reiniciar(d) }, casar(`reiniciar ${d.nome}`, t));
        add({ id: `t:${d.nome}`, titulo: `Terminar ${d.nome}`, detalhe: '', categoria: 'Ação', executar: () => terminar(d) }, casar(`terminar ${d.nome}`, t));
      } else {
        add({ id: `i:${d.nome}`, titulo: `Iniciar ${d.nome}`, detalhe: '', categoria: 'Ação', executar: () => iniciar(d) }, casar(`iniciar ${d.nome}`, t));
      }
    }

    if (t) {
      const processos: Item[] = [];
      for (const [distro, a] of Object.entries(estado.amostras)) {
        for (const p of a.processos) {
          const peso = String(p.pid) === t ? 3 : Math.max(casar(p.nome, t), p.cmdline.toLowerCase().includes(t) ? 1 : 0);
          if (!peso) continue;
          processos.push({
            id: `pr:${distro}:${p.pid}`,
            titulo: p.nome,
            detalhe: `PID ${p.pid} · ${distro} · CPU ${pct(p.cpu_pct)}`,
            categoria: 'Processo',
            peso,
            executar: () => {
              estado.foco = { distro, pid: p.pid };
              estado.pagina = 'processos';
            },
          });
        }
      }
      r.push(...processos.sort((a, b) => b.peso - a.peso).slice(0, PROCESSOS_NO_MAXIMO));

      if (/^\d+$/.test(t)) {
        for (const p of estado.portas.filter((p) => String(p.porta).startsWith(t))) {
          add(
            {
              id: `po:${p.protocolo}:${p.endereco}:${p.porta}`,
              titulo: `Porta ${p.porta}`,
              detalhe: `${p.nome ?? 'processo fora das distros'}${p.distro ? ` · ${p.distro}` : ''}`,
              categoria: 'Porta',
              executar: () => {
                estado.buscaPortas = String(p.porta);
                estado.pagina = 'portas';
              },
            },
            String(p.porta) === t ? 3 : 2,
          );
        }
      }
    }
    return r.sort((a, b) => b.peso - a.peso).slice(0, MAXIMO);
  });

  $effect(() => {
    if (estado.paletaAberta && !dialogo.open) {
      termo = '';
      ativo = 0;
      dialogo.showModal();
      campo?.focus();
    } else if (!estado.paletaAberta && dialogo.open) {
      dialogo.close();
    }
  });

  function escolher(i: Item | undefined) {
    if (!i) return;
    // Fecha antes de executar: a ação pode abrir a confirmação, que também é modal.
    estado.paletaAberta = false;
    i.executar();
  }

  function tecla(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      ativo = itens.length ? (ativo + 1) % itens.length : 0;
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      ativo = itens.length ? (ativo - 1 + itens.length) % itens.length : 0;
    } else if (e.key === 'Enter') {
      e.preventDefault();
      escolher(itens[ativo]);
    }
  }
</script>

<dialog
  bind:this={dialogo}
  onclose={() => (estado.paletaAberta = false)}
  onclick={(e) => e.target === dialogo && (estado.paletaAberta = false)}
  aria-label="Buscar"
>
  <input
    bind:this={campo}
    bind:value={termo}
    oninput={() => (ativo = 0)}
    onkeydown={tecla}
    type="text"
    role="combobox"
    aria-expanded="true"
    aria-controls="paleta-lista"
    aria-activedescendant={itens[ativo] ? `paleta-${ativo}` : undefined}
    aria-label="Buscar processo, porta, distro ou ação"
    placeholder="Processo, porta, distro ou ação"
    autocomplete="off"
    spellcheck="false"
  />
  <ul id="paleta-lista" role="listbox" aria-label="Resultados">
    {#each itens as item, i (item.id)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <li
        id="paleta-{i}"
        role="option"
        aria-selected={i === ativo}
        class:ativo={i === ativo}
        onclick={() => escolher(item)}
        onmousemove={() => (ativo = i)}
      >
        <span class="titulo">{item.titulo}</span>
        {#if item.detalhe}<span class="detalhe">{item.detalhe}</span>{/if}
        <span class="categoria">{item.categoria}</span>
      </li>
    {:else}
      <li class="nada" role="option" aria-selected="false" aria-disabled="true">Nada encontrado para "{termo}"</li>
    {/each}
  </ul>
  <p class="dica"><kbd>↑</kbd> <kbd>↓</kbd> para escolher · <kbd>Enter</kbd> para abrir · <kbd>Esc</kbd> para fechar</p>
</dialog>

<style>
  dialog {
    width: min(620px, calc(100vw - 32px));
    margin-top: 12vh;
    padding: 0;
    border: 1px solid var(--borda-forte);
    border-radius: 8px;
    background: var(--camada-forte);
    color: var(--texto);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.32);
  }

  dialog::backdrop {
    background: rgba(0, 0, 0, 0.25);
  }

  input {
    width: 100%;
    height: 48px;
    padding: 0 16px;
    border: 0;
    border-bottom: 1px solid var(--borda);
    outline: 0;
    background: transparent;
    font-size: 16px;
  }

  ul {
    max-height: min(420px, 60vh);
    margin: 0;
    padding: 4px;
    overflow: auto;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: baseline;
    gap: 10px;
    padding: 8px 12px;
    border-radius: 4px;
    cursor: default;
  }

  li.ativo {
    background: var(--selecionado);
  }

  .detalhe,
  .categoria,
  .nada {
    color: var(--texto-2);
    font-size: 12px;
  }

  .detalhe {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dica {
    margin: 0;
    padding: 8px 16px;
    border-top: 1px solid var(--borda);
    color: var(--texto-2);
    font-size: 12px;
  }

  kbd {
    font: inherit;
  }
</style>
