<script lang="ts">
  // Aviso que vale em qualquer página. O de RAM acabando fica só em Desempenho > Memória, e a
  // entrada Desempenho no menu ganha um sinal enquanto ele estiver ativo.
  import { desligarWsl, ocupadas } from './acoes.svelte';
  import { estado } from './estado.svelte';

  const hora = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' });
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
