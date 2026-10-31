<script lang="ts">
  // As maiores pastas da análise, em grupos: o total de cada um, a parte dele no que foi lido e
  // as pastas, as maiores primeiro. Grupo comprido mostra as primeiras e abre o resto a pedido.
  import { bytes } from './formato';
  import { organizar } from './pastas';
  import type { AnalisePastas } from './tipos';

  let { analise }: { analise: AnalisePastas } = $props();

  const VISIVEIS = 8;
  const grupos = $derived(organizar(analise));
  const total = $derived(grupos.reduce((s, g) => s + g.bytes, 0));
  let abertos = $state<Record<string, boolean>>({});

  const parte = (b: number) => (total ? (b / total) * 100 : 0);
  const pct = (b: number) => {
    const v = parte(b);
    return v >= 1 ? `${Math.round(v)}%` : '< 1%';
  };
</script>

<div class="grupos">
  {#each grupos as g (g.titulo)}
    {@const todos = abertos[g.titulo] || g.itens.length <= VISIVEIS + 2}
    <section class="grupo" aria-label={g.titulo}>
      <div class="topo">
        <h4>{g.titulo}</h4>
        <span class="quantos">{g.itens.length === 1 ? '1 pasta' : `${g.itens.length} pastas`}</span>
        <span class="numero total">{bytes(g.bytes)}</span>
        <span class="numero pct">{pct(g.bytes)}</span>
      </div>
      <div class="proporcao" aria-hidden="true"><span style:width="{parte(g.bytes)}%"></span></div>
      <p class="explica">{g.descricao}</p>
      <ul>
        {#each todos ? g.itens : g.itens.slice(0, VISIVEIS) as item (item.caminho)}
          <li title={item.caminho}>
            <span class="nome"><code>{item.nome}</code>{#if item.descricao}<span class="descricao">{item.descricao}</span>{/if}</span>
            <span class="numero">{bytes(item.bytes)}</span>
          </li>
        {/each}
      </ul>
      {#if !todos}
        <button class="botao sutil mais" onclick={() => (abertos[g.titulo] = true)}>
          Mostrar as outras {g.itens.length - VISIVEIS} ({bytes(g.itens.slice(VISIVEIS).reduce((s, i) => s + i.bytes, 0))})
        </button>
      {:else if g.itens.length > VISIVEIS + 2}
        <button class="botao sutil mais" onclick={() => (abertos[g.titulo] = false)}>Mostrar só as {VISIVEIS} maiores</button>
      {/if}
    </section>
  {/each}
</div>

<style>
  .grupos {
    display: grid;
    gap: 10px;
    margin: 8px 0 4px;
  }

  .grupo {
    padding: 10px 12px;
    border: 1px solid var(--borda);
    border-radius: var(--raio);
    background: var(--camada);
  }

  .topo {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }

  h4 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }

  .quantos {
    flex: 1;
    color: var(--texto-2);
    font-size: 12px;
  }

  .total {
    font-weight: 600;
  }

  .pct {
    min-width: 4ch;
    color: var(--texto-2);
    font-size: 12px;
    text-align: right;
  }

  .proporcao {
    height: 4px;
    margin: 6px 0;
    border-radius: 2px;
    background: var(--borda);
    overflow: hidden;
  }

  .proporcao span {
    display: block;
    height: 100%;
    background: var(--destaque);
  }

  .explica {
    margin: 0 0 6px;
    color: var(--texto-2);
    font-size: 12px;
  }

  ul {
    display: grid;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 13px;
  }

  li {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 2px 0;
  }

  .nome {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 10px;
    min-width: 0;
  }

  .nome code {
    overflow-wrap: anywhere;
  }

  .descricao {
    color: var(--texto-2);
    font-size: 12px;
  }

  .numero {
    flex: none;
    font-variant-numeric: tabular-nums;
  }

  .mais {
    margin-top: 4px;
    height: 26px;
    font-size: 12px;
  }
</style>
