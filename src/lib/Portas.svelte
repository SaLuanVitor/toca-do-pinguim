<script lang="ts">
  import { encerrarProcesso } from './acoes.svelte';
  import { estado, type PortaVista } from './estado.svelte';
  import Icone from './Icone.svelte';
  import { SIGTERM } from './tipos';

  const algumaRodando = $derived(estado.distros.some((d) => d.rodando));

  const portas = $derived.by(() => {
    const termo = estado.buscaPortas.trim().toLowerCase();
    return estado.portas
      .filter(
        (p) =>
          !termo ||
          String(p.porta).startsWith(termo) ||
          (p.nome ?? '').toLowerCase().includes(termo) ||
          (p.distro ?? '').toLowerCase().includes(termo),
      )
      .sort((a, b) => a.porta - b.porta || a.protocolo.localeCompare(b.protocolo));
  });

  function escuta(endereco: string): string {
    if (endereco === '0.0.0.0' || endereco === '::') return 'Todas as interfaces';
    if (endereco === '127.0.0.1' || endereco === '::1') return 'Só localhost';
    return endereco;
  }

  function verProcesso(p: PortaVista) {
    if (p.distro === null || p.pid === null) return;
    estado.foco = { distro: p.distro, pid: p.pid };
    estado.pagina = 'processos';
  }
</script>

<header class="barra">
  <h1>Portas</h1>
  <label class="busca">
    <Icone nome="busca" />
    <span class="visualmente-oculto">Filtrar portas</span>
    <input type="search" placeholder="Porta, processo ou distro" bind:value={estado.buscaPortas} />
  </label>
</header>

{#if !algumaRodando}
  <div class="vazio">
    <strong>Nenhuma distro em execução</strong>
    <span>As portas são lidas de dentro de uma distro ligada.</span>
  </div>
{:else if estado.portas.length === 0}
  <div class="vazio">
    <strong>Lendo as portas</strong>
    <span>A lista é atualizada a cada 5 amostras.</span>
  </div>
{:else}
  <div class="rolagem">
    <table aria-label="Portas em escuta na VM do WSL">
      <thead>
        <tr>
          <th scope="col" class="numero">Porta</th>
          <th scope="col">Protocolo</th>
          <th scope="col">Escuta em</th>
          <th scope="col">Processo</th>
          <th scope="col" class="numero">PID</th>
          <th scope="col">Distro</th>
          <th scope="col"><span class="visualmente-oculto">Ações</span></th>
        </tr>
      </thead>
      <tbody>
        {#each portas as p (`${p.protocolo}|${p.endereco}|${p.porta}`)}
          <tr>
            <th scope="row" class="numero">{p.porta}</th>
            <td>{p.protocolo.toUpperCase()}</td>
            <td title={p.endereco}>{escuta(p.endereco)}</td>
            <td>{p.nome ?? ''}</td>
            <td class="numero">{p.pid ?? ''}</td>
            <td class:fora={p.distro === null}>{p.distro ?? 'fora das distros'}</td>
            <td class="acoes-linha">
              {#if p.distro !== null && p.pid !== null}
                {@const distro = p.distro}
                {@const pid = p.pid}
                <button class="botao sutil" onclick={() => verProcesso(p)} aria-label="Ver processo da porta {p.porta}">
                  Ver processo
                </button>
                <button
                  class="botao sutil"
                  disabled={pid <= 1}
                  onclick={() => encerrarProcesso(distro, pid, p.nome ?? `PID ${pid}`, SIGTERM)}
                  aria-label="Encerrar {p.nome ?? 'processo'} na porta {p.porta}"
                >
                  <Icone nome="parar" /> Encerrar
                </button>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    {#if portas.length === 0}
      <p class="vazio">Nenhuma porta corresponde a "{estado.buscaPortas}".</p>
    {/if}
    <p class="nota">
      A rede é uma só para todas as distros. "Fora das distros" é um socket cujo processo
      nenhuma distro ligada enxerga, como o DNS interno do WSL em 10.255.255.254.
    </p>
  </div>
{/if}

<style>
  .busca {
    display: flex;
    align-items: center;
    gap: 8px;
    width: min(300px, 100%);
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--borda);
    border-bottom-color: var(--borda-forte);
    border-radius: var(--raio);
    background: var(--camada-forte);
    color: var(--texto-2);
  }

  .busca:focus-within {
    border-bottom: 2px solid var(--destaque);
  }

  .busca input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--texto);
  }

  .rolagem {
    flex: 1;
    overflow: auto;
    padding: 0 20px 20px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  th,
  td {
    height: 38px;
    padding: 0 10px;
    text-align: left;
    border-bottom: 1px solid var(--borda);
    white-space: nowrap;
  }

  thead th {
    height: 36px;
    color: var(--texto-2);
    font-weight: 400;
    font-size: 12px;
  }

  th.numero,
  td.numero {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  tbody th {
    font-weight: 600;
  }

  tbody tr:hover {
    background: var(--hover);
  }

  .fora {
    color: var(--texto-2);
  }

  .acoes-linha {
    text-align: right;
  }

  .acoes-linha .botao {
    height: 28px;
  }

  .nota {
    max-width: 70ch;
    color: var(--texto-2);
    font-size: 12px;
    line-height: 1.5;
  }
</style>
