<script lang="ts">
  // Só o que é processamento: uso de CPU da VM, o teto de processadores e quem gasta CPU.
  import ColunaOrdenavel from './ColunaOrdenavel.svelte';
  import { estado } from './estado.svelte';
  import { duracao, pct } from './formato';
  import Grafico from './Grafico.svelte';
  import LimiteProcessadores from './LimiteProcessadores.svelte';
  import { comparar, Ordenacao } from './ordenacao.svelte';

  type Coluna = 'nome' | 'processos' | 'cpu' | 'coleta';
  const ordem = new Ordenacao<Coluna>('processador-por-distro', 'cpu', (c) => c === 'nome');

  const vm = $derived(estado.vm);
  const cpu = $derived(estado.historico.map((p) => p.cpu));
  const porDistro = $derived(
    ordem.ordenar(
      Object.entries(estado.amostras).map(([nome, a]) => ({
        nome,
        processos: a.processos.length,
        cpu: a.processos.reduce((s, p) => s + p.cpu_pct, 0),
        coleta: a.agente.coleta_us,
      })),
      (d, c) => (c === 'cpu' ? Math.round(d.cpu * 10) : d[c]),
      (a, b) => comparar(a.nome, b.nome),
    ),
  );
  const totalProcessos = $derived(porDistro.reduce((s, d) => s + d.processos, 0));
</script>

{#if !vm}
  <div class="vazio">
    <strong>Nenhuma distro em execução</strong>
    <span>A VM do WSL só pode ser medida por dentro de uma distro ligada.</span>
    <button class="botao" onclick={() => (estado.pagina = 'distros')}>Ver distros</button>
  </div>
  <!-- O teto vale no próximo início, então faz sentido justo com o WSL desligado. -->
  <div class="rolagem">
    <LimiteProcessadores />
  </div>
{:else}
  <div class="rolagem">
    <section class="cartao" aria-labelledby="titulo-cpu">
      <div class="cabeca">
        <h2 id="titulo-cpu">Uso de CPU</h2>
        <span>VM do WSL · {vm.cpus} processadores</span>
      </div>
      <div class="area">
        <Grafico valores={cpu} cor="--cpu" rotulo="Uso de CPU nos últimos 60 segundos: {pct(vm.cpu_vm_pct)} agora" />
      </div>
      <dl>
        <div><dt>Utilização</dt><dd>{pct(vm.cpu_vm_pct)}</dd></div>
        <div><dt>Processos</dt><dd>{totalProcessos}</dd></div>
        <div><dt>Tempo ativo da VM</dt><dd>{duracao(vm.uptime_s)}</dd></div>
      </dl>
    </section>

    <LimiteProcessadores />

    <section class="cartao largo" aria-labelledby="titulo-cpu-distros">
      <div class="cabeca">
        <h2 id="titulo-cpu-distros">Por distro</h2>
        <span>Soma da CPU dos processos de cada uma; 100% é um processador inteiro</span>
      </div>
      <table>
        <thead>
          <tr>
            <ColunaOrdenavel {ordem} coluna="nome" titulo="Distro" />
            <ColunaOrdenavel {ordem} coluna="processos" titulo="Processos" numerica />
            <ColunaOrdenavel {ordem} coluna="cpu" titulo="CPU" numerica />
            <ColunaOrdenavel {ordem} coluna="coleta" titulo="Agente: tempo de leitura" numerica />
          </tr>
        </thead>
        <tbody>
          {#each porDistro as d (d.nome)}
            <tr>
              <th scope="row">{d.nome}</th>
              <td class="numero">{d.processos}</td>
              <td class="numero">{pct(d.cpu)}</td>
              <td class="numero">{(d.coleta / 1000).toLocaleString('pt-BR', { maximumFractionDigits: 1 })} ms</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  </div>
{/if}

<style>
  .rolagem {
    flex: 1;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    align-content: start;
    gap: 12px;
    padding: 0 20px 20px;
  }

  .cartao {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    border: 1px solid var(--borda);
    border-radius: 8px;
    background: var(--camada);
  }

  .cartao.largo {
    grid-column: 1 / -1;
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

  .area {
    height: 180px;
  }

  dl {
    display: flex;
    gap: 28px;
    flex-wrap: wrap;
    margin: 0;
  }

  dt {
    color: var(--texto-2);
    font-size: 12px;
  }

  dd {
    margin: 2px 0 0;
    font-size: 18px;
    font-variant-numeric: tabular-nums;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  table :global(th),
  td {
    height: 30px;
    padding: 0 10px;
    text-align: left;
    border-bottom: 1px solid var(--borda);
  }

  thead :global(th) {
    color: var(--texto-2);
    font-weight: 400;
    font-size: 12px;
  }

  tbody th {
    font-weight: 600;
  }

  table :global(th.numero),
  td.numero {
    text-align: right;
  }
</style>
