<script lang="ts">
  // Procura versão nova ao abrir e a cada 6 horas. Só baixa quando a pessoa clica: o app
  // gerencia o WSL e não deve fazer nada pesado por conta própria.
  import { onMount } from 'svelte';
  import { api } from './api';
  import { estado } from './estado.svelte';
  import type { Atualizacao } from './tipos';

  const A_CADA_MS = 6 * 60 * 60 * 1000;

  let nova = $state<Atualizacao | null>(null);
  let dispensada = $state<string | null>(null);
  let instalando = $state(false);

  async function procurar() {
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

  onMount(() => {
    procurar();
    const id = setInterval(procurar, A_CADA_MS);
    return () => clearInterval(id);
  });
</script>

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
