<script lang="ts">
  // Teto de processadores da VM do WSL, gravado como `processors` no .wslconfig.
  import { onMount } from 'svelte';
  import { api } from './api';
  import { estado } from './estado.svelte';

  let maquina = $state(0);
  // '' é o padrão do WSL: todos os processadores lógicos.
  let processadores = $state('');
  let gravado = $state('');
  let salvando = $state(false);

  onMount(async () => {
    try {
      const [limites, config] = await Promise.all([api.limitesDaMaquina(), api.lerWslconfig()]);
      maquina = limites.processadores;
      const atual = config.valores.find(([s, k]) => s.toLowerCase() === 'wsl2' && k.toLowerCase() === 'processors');
      processadores = gravado = atual ? atual[2].trim() : '';
    } catch (e) {
      estado.avisar(String(e));
    }
  });

  async function salvar(reiniciar: boolean) {
    salvando = true;
    try {
      await api.gravarWslconfig([{ secao: 'wsl2', chave: 'processors', valor: processadores || null }], reiniciar);
      gravado = processadores;
      if (reiniciar) estado.receberDistros(await api.listarDistros());
    } catch (e) {
      estado.avisar(String(e));
    } finally {
      salvando = false;
    }
  }

  function salvarEReiniciar() {
    const ligadas = estado.distros.filter((d) => d.rodando).map((d) => d.nome);
    estado.pedido = {
      titulo: 'Reiniciar o WSL agora?',
      texto: ligadas.length
        ? `Para o novo limite de processadores valer, a VM do WSL precisa reiniciar. Serão terminadas: ${ligadas.join(', ')}.`
        : 'Nenhuma distro está ligada; o novo limite vale na próxima vez que uma ligar.',
      acao: 'Salvar e reiniciar',
      executar: () => salvar(true),
    };
  }
</script>

<section class="cartao" aria-labelledby="titulo-limite-cpu">
  <div class="cabeca">
    <h2 id="titulo-limite-cpu">Limite de processadores</h2>
    <span>.wslconfig</span>
  </div>
  <label class="campo">
    <span>A VM pode usar</span>
    <select bind:value={processadores} disabled={!maquina}>
      <option value="">Todos ({maquina || '?'}, padrão do WSL)</option>
      {#each Array.from({ length: maquina }, (_, i) => String(i + 1)) as n (n)}
        <option value={n}>{n} de {maquina}</option>
      {/each}
    </select>
  </label>
  <p class="explica">
    Com menos processadores, um build ou um container que dispara não leva a máquina toda: a VM fica com até esse número
    de núcleos e o resto do Windows continua respondendo.{estado.vm ? ` Hoje a VM vê ${estado.vm.cpus}.` : ''} Vale quando o
    WSL reiniciar.
  </p>
  <div class="acoes">
    <button class="botao primario" disabled={processadores === gravado || salvando} onclick={() => salvar(false)}>
      Salvar para o próximo início
    </button>
    <button class="botao" disabled={processadores === gravado || salvando} onclick={salvarEReiniciar}>Salvar e reiniciar o WSL</button>
  </div>
</section>

<style>
  .cartao {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    border: 1px solid var(--borda);
    border-radius: 8px;
    background: var(--camada);
  }

  .cabeca {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }

  .cabeca span {
    color: var(--texto-2);
    font-size: 12px;
  }

  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }

  .campo {
    display: grid;
    gap: 6px;
  }

  .campo span {
    color: var(--texto-2);
    font-size: 12px;
  }

  .campo select {
    height: 32px;
    max-width: 240px;
    padding: 0 10px;
    border: 1px solid var(--borda);
    border-bottom-color: var(--borda-forte);
    border-radius: var(--raio);
    background: var(--camada-forte);
    color: var(--texto);
  }

  .explica {
    margin: 0;
    color: var(--texto-2);
    font-size: 12px;
  }

  .acoes {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: auto;
  }
</style>
