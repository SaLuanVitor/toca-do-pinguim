<script lang="ts">
  import { api } from './api';
  import { estado } from './estado.svelte';
  import { bytes } from './formato';
  import type { ResumoMemoria } from './tipos';

  let {
    resumo,
    aberto = $bindable(),
    onsalvo,
  }: { resumo: ResumoMemoria; aberto: boolean; onsalvo: () => void } = $props();

  const GIB = 1024 ** 3;
  let dialogo: HTMLDialogElement;
  let campo = $state<HTMLInputElement>();
  let gb = $state(0);
  let salvando = $state(false);

  const maximo = $derived(Math.floor(resumo.ram_host_bytes / GIB));
  const valido = $derived(Number.isInteger(gb) && gb >= 1 && gb <= maximo);
  const horas = $derived(Math.floor(resumo.minutos_de_dados / 60));

  $effect(() => {
    if (aberto && !dialogo.open) {
      gb = Math.round((resumo.sugestao_bytes ?? resumo.limite_bytes) / GIB);
      dialogo.showModal();
      campo?.select();
    } else if (!aberto && dialogo.open) {
      dialogo.close();
    }
  });

  async function salvar(reiniciar: boolean) {
    salvando = true;
    try {
      await api.definirLimiteMemoria(gb, reiniciar);
      aberto = false;
      onsalvo();
      if (reiniciar) estado.receberDistros(await api.listarDistros());
    } catch (e) {
      estado.avisar(String(e));
    } finally {
      salvando = false;
    }
  }

  function salvarEReiniciar() {
    const ligadas = estado.distros.filter((d) => d.rodando).map((d) => d.nome);
    aberto = false;
    estado.pedido = {
      titulo: 'Reiniciar o WSL agora?',
      texto: ligadas.length
        ? `Para o novo limite valer, a VM do WSL precisa reiniciar. Serão terminadas: ${ligadas.join(', ')}.`
        : 'Nenhuma distro está ligada; o novo limite vale na próxima vez que uma ligar.',
      acao: 'Salvar e reiniciar',
      executar: () => salvar(true),
    };
  }
</script>

<dialog bind:this={dialogo} onclose={() => (aberto = false)} aria-labelledby="limite-titulo">
  <h2 id="limite-titulo">Limite de memória do WSL</h2>
  <div class="corpo">
    <p>
      Hoje: <strong>{bytes(resumo.limite_bytes)}</strong>
      {resumo.limite_configurado ? 'definido no .wslconfig' : '(padrão do WSL: metade da RAM do Windows)'}.
      A VM não passa desse valor: acima dele o Linux usa o swap e, sem swap livre, encerra processos.
    </p>
    {#if resumo.sugestao_bytes !== null}
      <p>
        Nos últimos {horas >= 24 ? `${Math.floor(horas / 24)} dias` : `${horas} h`} de uso medidos, o pico foi
        <strong>{bytes(resumo.pico_bytes)}</strong>, sem contar o cache que o Linux devolve. A sugestão é esse pico
        com 25% de folga: <strong>{bytes(resumo.sugestao_bytes)}</strong>.
      </p>
    {:else}
      <p>Ainda não há uma hora de uso medida; a sugestão aparece depois disso.</p>
    {/if}
    <label class="campo">
      <span>Novo limite (GB, de 1 a {maximo})</span>
      <input type="number" min="1" max={maximo} step="1" bind:value={gb} bind:this={campo} aria-invalid={!valido} />
    </label>
    <p class="nota">O valor vai para o .wslconfig e só vale quando a VM do WSL reiniciar.</p>
  </div>
  <div class="rodape">
    <button class="botao primario" disabled={!valido || salvando} onclick={() => salvar(false)}>
      Salvar para o próximo início
    </button>
    <button class="botao" disabled={!valido || salvando} onclick={salvarEReiniciar}>Salvar e reiniciar o WSL</button>
    <button class="botao" onclick={() => (aberto = false)}>Cancelar</button>
  </div>
</dialog>

<style>
  dialog {
    width: min(520px, calc(100vw - 32px));
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
    margin: 24px 24px 8px;
  }

  .corpo {
    padding: 0 24px 16px;
    line-height: 1.45;
  }

  .campo {
    display: grid;
    gap: 6px;
    max-width: 220px;
  }

  .campo span {
    color: var(--texto-2);
    font-size: 12px;
  }

  .campo input {
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--borda);
    border-bottom-color: var(--borda-forte);
    border-radius: var(--raio);
    background: var(--camada-forte);
  }

  .campo input[aria-invalid='true'] {
    border-bottom: 2px solid var(--perigo);
  }

  .nota {
    color: var(--texto-2);
    font-size: 12px;
  }

  .rodape {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 20px 24px;
    background: var(--hover);
    border-top: 1px solid var(--borda);
  }
</style>
