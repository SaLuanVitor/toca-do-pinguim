<script lang="ts">
  // Uma entrada no menu, três abas: cada uma só com o que é dela, processamento, RAM ou disco.
  import Disco from './Disco.svelte';
  import { estado } from './estado.svelte';
  import Memoria from './Memoria.svelte';
  import Processador from './Processador.svelte';
  import type { AbaDesempenho } from './tipos';

  const ABAS: { id: AbaDesempenho; titulo: string }[] = [
    { id: 'processamento', titulo: 'Processamento' },
    { id: 'memoria', titulo: 'Memória' },
    { id: 'disco', titulo: 'Disco' },
  ];

  let botoes: HTMLButtonElement[] = [];

  // Setas mudam de aba e levam o foco junto, como pede o padrão de abas da ARIA.
  function tecla(e: KeyboardEvent, i: number) {
    const destino =
      e.key === 'ArrowRight' ? (i + 1) % ABAS.length
      : e.key === 'ArrowLeft' ? (i - 1 + ABAS.length) % ABAS.length
      : e.key === 'Home' ? 0
      : e.key === 'End' ? ABAS.length - 1
      : null;
    if (destino === null) return;
    e.preventDefault();
    estado.abaDesempenho = ABAS[destino].id;
    botoes[destino]?.focus();
  }
</script>

<header class="barra">
  <h1>Desempenho</h1>
</header>

<div class="abas" role="tablist" aria-label="Recurso">
  {#each ABAS as aba, i (aba.id)}
    <button
      bind:this={botoes[i]}
      id="aba-{aba.id}"
      role="tab"
      aria-selected={estado.abaDesempenho === aba.id}
      aria-controls="painel-desempenho"
      tabindex={estado.abaDesempenho === aba.id ? 0 : -1}
      onclick={() => (estado.abaDesempenho = aba.id)}
      onkeydown={(e) => tecla(e, i)}
    >
      {aba.titulo}
      {#if aba.id === 'memoria' && estado.memoriaWindows?.pressao}
        <span class="alerta" role="img" aria-label="RAM do Windows acabando"></span>
      {/if}
    </button>
  {/each}
</div>

<div class="painel" id="painel-desempenho" role="tabpanel" aria-labelledby="aba-{estado.abaDesempenho}">
  {#if estado.abaDesempenho === 'processamento'}
    <Processador />
  {:else if estado.abaDesempenho === 'memoria'}
    <Memoria />
  {:else}
    <Disco />
  {/if}
</div>

<style>
  .abas {
    display: flex;
    gap: 4px;
    padding: 0 20px;
    margin-bottom: 12px;
    border-bottom: 1px solid var(--borda);
  }

  [role='tab'] {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 34px;
    padding: 0 14px;
    border: 0;
    border-radius: var(--raio) var(--raio) 0 0;
    background: transparent;
    color: var(--texto-2);
  }

  [role='tab']:hover {
    background: var(--hover);
    color: var(--texto);
  }

  [role='tab'][aria-selected='true'] {
    color: var(--texto);
    font-weight: 600;
  }

  [role='tab'][aria-selected='true']::after {
    content: '';
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: -1px;
    height: 3px;
    border-radius: 2px;
    background: var(--destaque);
  }

  .alerta {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: rgb(var(--calor));
  }

  .painel {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
</style>
