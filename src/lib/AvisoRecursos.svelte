<script lang="ts">
  import { desligarWsl, ocupadas } from './acoes.svelte';
  import { estado } from './estado.svelte';
  import { bytes } from './formato';
  import { agrupar } from './projetos';

  const hora = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' });

  const memoria = $derived(estado.memoriaWindows);
  const maior = $derived(agrupar(estado.docker.conteineres).find((p) => p.memoria > 0));
  // Quando o cache passa da soma dos containers, apontar o maior container engana: a regra
  // é a mesma da notificação, em recursos.rs.
  const cache = $derived(estado.vm ? Math.max(0, estado.vm.mem_disponivel - estado.vm.mem_livre) : null);
  const docker = $derived(estado.docker.conteineres.reduce((s, c) => s + c.memoria_bytes, 0));
</script>

{#if estado.saude}
  <div class="aviso perigo" role="alert">
    <span>
      <strong>O WSL não está respondendo</strong> desde {hora.format(estado.saude.desde_ms)}{estado.saude.alvo
        ? ` em ${estado.saude.alvo}`
        : ''}: {estado.saude.motivo}. Desligar o WSL o destrava; todas as distros e os containers param.
    </span>
    <button class="botao primario" onclick={desligarWsl} disabled={!!ocupadas.WSL}>{ocupadas.WSL ?? 'Desligar o WSL'}</button>
  </div>
{/if}

{#if memoria?.pressao}
  <div class="aviso" role="status">
    <span>
      <strong>A memória do Windows está acabando:</strong>
      {bytes(memoria.disponivel_bytes)} livres de {bytes(memoria.total_bytes)}.
      {#if memoria.wsl_bytes !== null}
        {#if cache !== null && cache >= docker}
          O WSL ocupa {bytes(memoria.wsl_bytes)}, e a maior parte é cache de arquivos ({bytes(cache)}), que o Linux devolve
          quando precisa mas o Windows conta como ocupado. Baixar o limite de memória do WSL segura esse tamanho.
        {:else}
          O WSL ocupa {bytes(memoria.wsl_bytes)}{#if maior}; quem mais usa dentro dele é {maior.nome}, com {bytes(maior.memoria)}{/if}.
        {/if}
      {/if}
    </span>
    {#if memoria.wsl_bytes !== null && cache !== null && cache >= docker}
      <button
        class="botao"
        onclick={() => {
          estado.pagina = 'desempenho';
          estado.ajustarLimite = true;
        }}>Ajustar limite</button
      >
    {/if}
    {#if estado.pagina !== 'desempenho'}
      <button class="botao" onclick={() => (estado.pagina = 'desempenho')}>Ver o que ocupa</button>
    {/if}
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
    border-left: 3px solid rgb(var(--calor));
    border-radius: var(--raio);
    background: rgba(var(--calor), 0.12);
    font-size: 13px;
  }

  .aviso.perigo {
    border-left-color: var(--perigo);
    background: var(--camada-forte);
  }

  .aviso span {
    flex: 1;
    min-width: 200px;
  }
</style>
