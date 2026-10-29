<script lang="ts">
  import { estado } from './estado.svelte';

  let dialogo: HTMLDialogElement;
  let cancelar = $state<HTMLButtonElement>();

  $effect(() => {
    if (estado.pedido && !dialogo.open) {
      dialogo.showModal();
      // O foco abre em Cancelar: Enter por engano não encerra nada.
      cancelar?.focus();
    } else if (!estado.pedido && dialogo.open) {
      dialogo.close();
    }
  });

  function confirmar() {
    const p = estado.pedido;
    estado.pedido = null;
    p?.executar();
  }
</script>

<dialog bind:this={dialogo} onclose={() => (estado.pedido = null)} aria-labelledby="confirmar-titulo">
  {#if estado.pedido}
    <h2 id="confirmar-titulo">{estado.pedido.titulo}</h2>
    <p>{estado.pedido.texto}</p>
    <div class="rodape">
      <button class="botao primario" onclick={confirmar}>{estado.pedido.acao}</button>
      <button class="botao" bind:this={cancelar} onclick={() => (estado.pedido = null)}>Cancelar</button>
    </div>
  {/if}
</dialog>

<style>
  dialog {
    width: min(440px, calc(100vw - 32px));
    padding: 0;
    border: 1px solid var(--borda-forte);
    border-radius: 8px;
    background: var(--camada-forte);
    color: var(--texto);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.28);
  }

  dialog::backdrop {
    background: rgba(0, 0, 0, 0.3);
  }

  h2 {
    font-size: 20px;
    font-weight: 600;
    margin: 24px 24px 12px;
  }

  p {
    margin: 0 24px 24px;
    line-height: 1.45;
  }

  .rodape {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    padding: 20px 24px;
    background: var(--hover);
    border-top: 1px solid var(--borda);
  }

  .rodape .botao {
    justify-content: center;
  }
</style>
