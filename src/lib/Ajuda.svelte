<script lang="ts">
  // Um "?" ao lado de algo que pede explicação. Abre com clique ou Enter e fecha com Esc, com
  // outro clique ou quando o foco sai dele.
  let { assunto, texto }: { assunto: string; texto: string } = $props();

  let aberta = $state(false);
  const id = `ajuda-${Math.random().toString(36).slice(2, 9)}`;
</script>

<span
  class="ajuda"
  onfocusout={(e) => {
    if (!e.currentTarget.contains(e.relatedTarget as Node | null)) aberta = false;
  }}
>
  <button
    type="button"
    class="pergunta"
    aria-label="O que é {assunto}?"
    aria-expanded={aberta}
    aria-controls={id}
    onclick={() => (aberta = !aberta)}
    onkeydown={(e) => {
      if (e.key === 'Escape') aberta = false;
    }}>?</button
  >
  {#if aberta}
    <span {id} class="balao" role="note">{texto}</span>
  {/if}
</span>

<style>
  .ajuda {
    position: relative;
    display: inline-flex;
    vertical-align: middle;
  }

  .pergunta {
    width: 20px;
    height: 20px;
    padding: 0;
    border: 1px solid var(--borda-forte);
    border-radius: 50%;
    background: var(--camada-forte);
    color: var(--texto-2);
    font-size: 12px;
    font-weight: 600;
    line-height: 1;
    cursor: pointer;
  }

  .pergunta:hover,
  .pergunta[aria-expanded='true'] {
    color: var(--texto);
    border-color: var(--destaque);
  }

  .balao {
    position: absolute;
    z-index: 10;
    top: calc(100% + 6px);
    left: 0;
    width: min(340px, 70vw);
    padding: 10px 12px;
    border: 1px solid var(--borda-forte);
    border-radius: var(--raio);
    background: var(--camada-forte);
    color: var(--texto);
    font-size: 12px;
    font-weight: 400;
    line-height: 1.45;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.25);
  }
</style>
