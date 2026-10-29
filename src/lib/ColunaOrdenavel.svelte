<script lang="ts" generics="C extends string">
  import type { Ordenacao } from './ordenacao.svelte';

  let {
    ordem,
    coluna,
    titulo,
    numerica = false,
    total,
    classe = '',
    dica,
  }: {
    ordem: Ordenacao<C>;
    coluna: C;
    titulo: string;
    numerica?: boolean;
    /** Valor grande acima do título, como o total de CPU no Gerenciador de Tarefas. */
    total?: string;
    classe?: string;
    dica?: string;
  } = $props();
</script>

<th scope="col" class="{classe} ordenavel" class:numero={numerica} title={dica} aria-sort={ordem.ariaSort(coluna)}>
  <button onclick={() => ordem.alternar(coluna)}>
    {#if total !== undefined}<span class="total">{total}</span>{/if}
    <span class="titulo">{titulo}</span>
  </button>
</th>

<style>
  /* Duas classes para vencer o `table th` de quem usa o componente: o espaço fica no botão. */
  th.ordenavel {
    padding: 0;
  }

  button {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: flex-start;
    width: 100%;
    height: 100%;
    min-height: 36px;
    padding: 6px 10px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: inherit;
  }

  th.numero button {
    align-items: flex-end;
  }

  button:hover {
    background: var(--hover);
  }

  .total {
    color: var(--texto);
    font-size: 16px;
    font-variant-numeric: tabular-nums;
  }

  /* Título longo quebra em duas linhas em vez de alargar a tabela e cortar as ações. */
  .titulo {
    color: var(--texto-2);
    font-size: 12px;
    white-space: normal;
    line-height: 1.25;
  }

  /* O espaço que não quebra mantém a seta junto da última palavra do título. */
  [aria-sort='none'] .titulo::after {
    content: '\00a0▲';
    font-size: 9px;
    visibility: hidden;
  }

  [aria-sort='ascending'] .titulo::after {
    content: '\00a0▲';
    font-size: 9px;
  }

  [aria-sort='descending'] .titulo::after {
    content: '\00a0▼';
    font-size: 9px;
  }

  [aria-sort='ascending'] .titulo,
  [aria-sort='descending'] .titulo {
    color: var(--texto);
    font-weight: 600;
  }
</style>
