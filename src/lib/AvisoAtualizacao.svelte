<script lang="ts">
  // Procura versão nova ao abrir, a cada hora e quando a pessoa volta para a janela, se não
  // desligou a busca em Configurações > Este app. Com o app aberto o dia todo, a busca de 6
  // em 6 horas deixava a versão nova escondida até reabrir. Só baixa quando ela clica: o app
  // gerencia o WSL e não deve fazer nada pesado por conta própria.
  import { onMount } from 'svelte';
  import { api } from './api';
  import { estado } from './estado.svelte';
  import type { Atualizacao } from './tipos';

  const A_CADA_MS = 60 * 60 * 1000;
  /** Ao voltar para a janela, procura de novo só se a última busca passou disto. */
  const AO_VOLTAR_MS = 15 * 60 * 1000;

  let ultima = 0;

  let nova = $state<Atualizacao | null>(null);
  let dispensada = $state<string | null>(null);
  let instalando = $state(false);

  async function procurar() {
    if (!estado.procurarAtualizacoes) return;
    ultima = Date.now();
    try {
      nova = await api.procurarAtualizacao();
    } catch {
      // Sem rede ou sem release com latest.json: tenta de novo na próxima rodada, sem aviso.
    }
  }

  async function instalar() {
    instalando = true;
    try {
      // Dá certo fechando o app; só volta para cá se falhar.
      await api.instalarAtualizacao();
    } catch (e) {
      estado.avisar(`Atualização não instalada: ${e}`);
    } finally {
      instalando = false;
    }
  }

  async function abrirPagina() {
    if (!nova) return;
    try {
      await api.abrirPaginaDaVersao(nova.versao);
    } catch (e) {
      estado.avisar(String(e));
    }
  }

  function aoVoltar() {
    if (document.visibilityState === 'visible' && Date.now() - ultima >= AO_VOLTAR_MS) procurar();
  }

  // Religar a busca procura na hora, sem esperar a próxima rodada.
  $effect(() => {
    if (estado.procurarAtualizacoes) procurar();
    else nova = null;
  });

  onMount(() => {
    const id = setInterval(procurar, A_CADA_MS);
    return () => clearInterval(id);
  });
</script>

<svelte:window onfocus={aoVoltar} />
<svelte:document onvisibilitychange={aoVoltar} />

{#if nova && dispensada !== nova.versao}
  <div class="aviso" role="status">
    <span>
      Versão {nova.versao} disponível.
      {#if !nova.instalavel}Esta é a versão portátil: baixe a nova na página da versão.{/if}
    </span>
    {#if nova.instalavel}
      <button class="botao primario" onclick={instalar} disabled={instalando}>
        {instalando ? 'Baixando' : 'Atualizar e reiniciar'}
      </button>
    {/if}
    <button class="botao" onclick={abrirPagina}>O que mudou</button>
    <button class="botao sutil" onclick={() => (dispensada = nova?.versao ?? null)} disabled={instalando}>
      Agora não
    </button>
  </div>
{/if}

<style>
  .aviso {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 12px 20px 0;
    padding: 8px 8px 8px 12px;
    border: 1px solid var(--borda);
    border-left: 3px solid var(--destaque);
    border-radius: var(--raio);
    background: var(--selecionado);
    font-size: 13px;
  }

  .aviso span {
    flex: 1;
    min-width: 200px;
  }
</style>
