<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from './api';
  import { estado } from './estado.svelte';
  import { bytes, duracao, pct } from './formato';
  import Grafico from './Grafico.svelte';
  import LimiteMemoria from './LimiteMemoria.svelte';
  import type { ResumoMemoria } from './tipos';

  let resumo = $state<ResumoMemoria | null>(null);
  let ajustando = $state(false);

  async function atualizarResumo() {
    try {
      resumo = await api.resumoMemoria();
    } catch (e) {
      estado.avisar(String(e));
    }
  }

  // A busca pode pedir o ajuste direto; ele abre assim que o resumo estiver carregado.
  $effect(() => {
    if (estado.ajustarLimite && resumo) {
      estado.ajustarLimite = false;
      ajustando = true;
    }
  });

  // O histórico fecha um ponto por minuto; consultar mais vezes que isso não muda nada.
  onMount(() => {
    atualizarResumo();
    const t = setInterval(atualizarResumo, 60_000);
    return () => clearInterval(t);
  });

  const vm = $derived(estado.vm);
  const cpu = $derived(estado.historico.map((p) => p.cpu));
  const mem = $derived(estado.historico.map((p) => p.mem));
  const memUsada = $derived(vm ? vm.mem_total - vm.mem_disponivel : 0);
  const swapUsada = $derived(vm ? vm.swap_total - vm.swap_livre : 0);
  const porDistro = $derived(
    Object.entries(estado.amostras)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([nome, a]) => ({
        nome,
        processos: a.processos.length,
        cpu: a.processos.reduce((s, p) => s + p.cpu_pct, 0),
        mem: a.processos.reduce((s, p) => s + p.rss, 0),
        agente: a.agente,
      })),
  );
  const totalProcessos = $derived(porDistro.reduce((s, d) => s + d.processos, 0));
</script>

<header class="barra">
  <h1>Desempenho</h1>
</header>

{#if !vm}
  <div class="vazio">
    <strong>Nenhuma distro em execução</strong>
    <span>A VM do WSL só pode ser medida por dentro de uma distro ligada.</span>
    <button class="botao" onclick={() => (estado.pagina = 'distros')}>Ver distros</button>
  </div>
{:else}
  <div class="rolagem">
    <section class="cartao" aria-labelledby="titulo-cpu">
      <div class="cabeca">
        <h2 id="titulo-cpu">CPU</h2>
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

    <section class="cartao" aria-labelledby="titulo-mem">
      <div class="cabeca">
        <h2 id="titulo-mem">Memória</h2>
        <span>{bytes(vm.mem_total)} visíveis para a VM</span>
      </div>
      <div class="area">
        <Grafico
          valores={mem}
          cor="--mem"
          rotulo="Memória em uso nos últimos 60 segundos: {bytes(memUsada)} de {bytes(vm.mem_total)}"
        />
      </div>
      <dl>
        <div><dt>Em uso</dt><dd>{bytes(memUsada)}</dd></div>
        <div><dt>Disponível</dt><dd>{bytes(vm.mem_disponivel)}</dd></div>
        <div><dt>Swap em uso</dt><dd>{bytes(swapUsada)} de {bytes(vm.swap_total)}</dd></div>
      </dl>
      {#if resumo}
        <div class="limite">
          <dl>
            <div>
              <dt>Limite {resumo.limite_configurado ? '(.wslconfig)' : '(padrão do WSL)'}</dt>
              <dd>{bytes(resumo.limite_bytes)}</dd>
            </div>
            <div>
              <dt>Pico medido</dt>
              <dd>{resumo.minutos_de_dados ? bytes(resumo.pico_bytes) : 'sem dados ainda'}</dd>
            </div>
            <div>
              <dt>Sugestão</dt>
              <dd>{resumo.sugestao_bytes !== null ? bytes(resumo.sugestao_bytes) : 'após 1 h de uso'}</dd>
            </div>
          </dl>
          <button class="botao" onclick={() => (ajustando = true)}>Ajustar limite</button>
        </div>
      {/if}
    </section>

    <section class="cartao largo" aria-labelledby="titulo-distros">
      <div class="cabeca">
        <h2 id="titulo-distros">Por distro</h2>
        <span>Soma dos processos de cada uma; o agente aparece separado</span>
      </div>
      <table>
        <thead>
          <tr>
            <th scope="col">Distro</th>
            <th scope="col" class="numero">Processos</th>
            <th scope="col" class="numero">CPU</th>
            <th scope="col" class="numero">Memória</th>
            <th scope="col" class="numero">Agente: memória</th>
            <th scope="col" class="numero">Agente: leitura</th>
          </tr>
        </thead>
        <tbody>
          {#each porDistro as d (d.nome)}
            <tr>
              <th scope="row">{d.nome}</th>
              <td class="numero">{d.processos}</td>
              <td class="numero">{pct(d.cpu)}</td>
              <td class="numero">{bytes(d.mem)}</td>
              <td class="numero">{bytes(d.agente.rss)}</td>
              <td class="numero">{(d.agente.coleta_us / 1000).toLocaleString('pt-BR', { maximumFractionDigits: 1 })} ms</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  </div>
{/if}

{#if resumo}
  <LimiteMemoria {resumo} bind:aberto={ajustando} onsalvo={atualizarResumo} />
{/if}

<style>
  .limite {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
    padding-top: 12px;
    border-top: 1px solid var(--borda);
  }

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

  th,
  td {
    height: 30px;
    padding: 0 10px;
    text-align: left;
    border-bottom: 1px solid var(--borda);
  }

  thead th {
    color: var(--texto-2);
    font-weight: 400;
    font-size: 12px;
  }

  tbody th {
    font-weight: 600;
  }

  th.numero,
  td.numero {
    text-align: right;
  }
</style>
