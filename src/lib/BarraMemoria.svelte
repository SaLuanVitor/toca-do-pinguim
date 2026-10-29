<script lang="ts">
  import { bytes } from './formato';

  interface Parte {
    rotulo: string;
    bytes: number;
    /** Cor CSS completa, como `var(--mem)`. */
    cor: string;
  }

  let { titulo, partes }: { titulo: string; partes: Parte[] } = $props();

  const total = $derived(partes.reduce((s, p) => s + Math.max(0, p.bytes), 0));
  const descricao = $derived(partes.map((p) => `${p.rotulo}: ${bytes(Math.max(0, p.bytes))}`).join(', '));
</script>

<figure>
  <figcaption>{titulo}</figcaption>
  <div class="barra-memoria" role="img" aria-label="{titulo}: {descricao}">
    {#each partes as p (p.rotulo)}
      {#if p.bytes > 0 && total > 0}
        <span style:width="{(p.bytes / total) * 100}%" style:background={p.cor}></span>
      {/if}
    {/each}
  </div>
  <ul>
    {#each partes as p (p.rotulo)}
      <li><i style:background={p.cor}></i>{p.rotulo} <strong>{bytes(Math.max(0, p.bytes))}</strong></li>
    {/each}
  </ul>
</figure>

<style>
  figure {
    display: grid;
    gap: 6px;
    margin: 0;
  }

  figcaption {
    color: var(--texto-2);
    font-size: 12px;
  }

  .barra-memoria {
    display: flex;
    height: 14px;
    overflow: hidden;
    border-radius: 4px;
    background: var(--hover);
  }

  .barra-memoria span + span {
    border-left: 1px solid var(--camada-forte);
  }

  ul {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 12px;
  }

  li {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  i {
    width: 10px;
    height: 10px;
    border-radius: 2px;
  }

  strong {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
</style>
