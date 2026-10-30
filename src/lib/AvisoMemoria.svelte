<script lang="ts">
  import { estado } from './estado.svelte';
  import { bytes } from './formato';
  import { explicarPressao } from './pressao';

  let { onajustar }: { onajustar: () => void } = $props();

  const memoria = $derived(estado.memoriaWindows);
  const explicacao = $derived(
    memoria
      ? explicarPressao({
          total: memoria.total_bytes,
          disponivel: memoria.disponivel_bytes,
          wsl: memoria.wsl_bytes,
          cache: estado.vm ? Math.max(0, estado.vm.mem_disponivel - estado.vm.mem_livre) : null,
          conteineres: estado.docker.conteineres,
        })
      : null,
  );
</script>

{#if memoria?.pressao && explicacao}
  <div class="aviso" role="status">
    <span>
      <strong>A RAM do Windows está acabando:</strong>
      {bytes(memoria.disponivel_bytes)} livres de {bytes(memoria.total_bytes)}.
      {explicacao.texto}
    </span>
    {#if explicacao.culpaDoCache}
      <button class="botao" onclick={onajustar}>Ajustar limite</button>
    {/if}
  </div>
{/if}

<style>
  .aviso {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 0 20px 12px;
    padding: 8px 8px 8px 12px;
    border: 1px solid var(--borda);
    border-left: 3px solid rgb(var(--calor));
    border-radius: var(--raio);
    background: rgba(var(--calor), 0.12);
    font-size: 13px;
  }

  .aviso span {
    flex: 1;
    min-width: 240px;
  }
</style>
