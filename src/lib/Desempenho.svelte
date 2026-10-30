<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from './api';
  import BarraMemoria from './BarraMemoria.svelte';
  import ColunaOrdenavel from './ColunaOrdenavel.svelte';
  import { estado } from './estado.svelte';
  import { bytes, duracao, pct } from './formato';
  import Grafico from './Grafico.svelte';
  import LimiteMemoria from './LimiteMemoria.svelte';
  import { comparar, Ordenacao } from './ordenacao.svelte';
  import ProjetosDocker from './ProjetosDocker.svelte';
  import type { Devolucao, ResumoMemoria } from './tipos';

  type Coluna = 'nome' | 'processos' | 'cpu' | 'mem' | 'agenteRss' | 'coleta';
  const ordem = new Ordenacao<Coluna>('por-distro', 'nome', (c) => c === 'nome');

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
  // Agente antigo não manda as três partes; aí o cache sai da diferença entre disponível e livre.
  const partes = $derived(vm ? vm.mem_blocos + vm.mem_arquivos + vm.mem_tabelas : 0);
  const cache = $derived(vm ? (partes > 0 ? partes : Math.max(0, vm.mem_disponivel - vm.mem_livre)) : 0);
  const programas = $derived(vm ? (partes > 0 ? Math.max(0, vm.mem_total - vm.mem_livre - partes) : memUsada) : 0);
  // O que passa da sugestão no teto da VM vira cache, que o Windows paga.
  const sobra = $derived(
    resumo && resumo.sugestao_bytes !== null && resumo.limite_bytes > resumo.sugestao_bytes && cache > programas
      ? resumo.limite_bytes - resumo.sugestao_bytes
      : 0,
  );
  const devolucao = $derived(estado.devolucao);
  // O esvaziamento leva alguns segundos antes de o backend avisar que começou.
  let pedindo = $state(false);
  const devolvendo = $derived(pedindo || (!!devolucao && !devolucao.concluida));
  const hora = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' });

  function resultado(d: Devolucao): string {
    if (!d.concluida) {
      return d.logo_bytes === null
        ? `Devolvendo: a VM tinha ${bytes(d.antes_bytes)} na RAM do Windows. O resultado sai em 2 min.`
        : `15 s depois: de ${bytes(d.antes_bytes)} para ${bytes(d.logo_bytes)}. O resultado final sai em 2 min.`;
    }
    const depois = d.depois_bytes ?? d.antes_bytes;
    const quem = d.automatica ? 'Devolvido sozinho' : 'Devolvido';
    return `${quem} às ${hora.format(d.quando_ms)}: de ${bytes(d.antes_bytes)} para ${bytes(d.logo_bytes ?? depois)} em 15 s; 2 min depois, ${bytes(depois)}, ${bytes(Math.max(0, d.antes_bytes - depois))} a menos que antes.`;
  }

  async function devolver() {
    pedindo = true;
    try {
      await api.devolverCache();
    } catch (e) {
      estado.avisar(String(e));
    } finally {
      pedindo = false;
    }
  }
  const windows = $derived(estado.memoriaWindows);
  const usadaWindows = $derived(windows ? windows.total_bytes - windows.disponivel_bytes : 0);
  const porDistro = $derived(
    ordem.ordenar(
      Object.entries(estado.amostras).map(([nome, a]) => ({
        nome,
        processos: a.processos.length,
        cpu: a.processos.reduce((s, p) => s + p.cpu_pct, 0),
        mem: a.processos.reduce((s, p) => s + p.rss, 0),
        agente: a.agente,
      })),
      (d, c) =>
        c === 'cpu' ? Math.round(d.cpu * 10) : c === 'agenteRss' ? d.agente.rss : c === 'coleta' ? d.agente.coleta_us : d[c],
      (a, b) => comparar(a.nome, b.nome),
    ),
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
          rotulo="Memória usada por programas e kernel nos últimos 60 segundos: {bytes(memUsada)} de {bytes(vm.mem_total)}"
        />
      </div>
      <dl>
        <div><dt>Programas e kernel</dt><dd>{bytes(programas)}</dd></div>
        <div><dt>Cache descartável</dt><dd>{bytes(cache)}</dd></div>
        {#if windows?.wsl_bytes != null}
          <div><dt>Na RAM do Windows</dt><dd>{bytes(windows.wsl_bytes)}</dd></div>
        {/if}
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

    {#if windows}
      <section class="cartao largo" aria-labelledby="titulo-onde">
        <div class="cabeca">
          <h2 id="titulo-onde">Para onde vai a memória</h2>
          <span>Comprometida no Windows: {bytes(windows.commit_bytes)} de {bytes(windows.commit_limite_bytes)} (RAM mais arquivo de paginação)</span>
        </div>
        <BarraMemoria
          titulo="Windows: {bytes(windows.total_bytes)} de RAM"
          partes={[
            { rotulo: 'VM do WSL', bytes: windows.wsl_bytes ?? 0, cor: 'var(--mem)' },
            { rotulo: 'Outros programas', bytes: usadaWindows - (windows.wsl_bytes ?? 0), cor: 'var(--cpu)' },
            { rotulo: 'Disponível', bytes: windows.disponivel_bytes, cor: 'var(--borda-forte)' },
          ]}
        />
        <BarraMemoria
          titulo="Dentro da VM do WSL: {bytes(vm.mem_total)}"
          partes={partes > 0
            ? [
                { rotulo: 'Programas e kernel', bytes: programas, cor: 'var(--mem)' },
                { rotulo: 'Tabelas de arquivos', bytes: vm.mem_tabelas, cor: 'rgb(var(--calor))' },
                { rotulo: 'Blocos de disco', bytes: vm.mem_blocos, cor: 'rgba(var(--calor), 0.65)' },
                { rotulo: 'Arquivos lidos', bytes: vm.mem_arquivos, cor: 'rgba(var(--calor), 0.4)' },
                { rotulo: 'Livre', bytes: vm.mem_livre, cor: 'var(--borda-forte)' },
              ]
            : [
                { rotulo: 'Programas e kernel', bytes: programas, cor: 'var(--mem)' },
                { rotulo: 'Cache descartável', bytes: cache, cor: 'rgb(var(--calor))' },
                { rotulo: 'Livre', bytes: vm.mem_livre, cor: 'var(--borda-forte)' },
              ]}
        />
        <p class="nota">
          O Linux guarda o que leu do disco: as tabelas com o nome e o tamanho de cada arquivo percorrido, os blocos do disco e o
          conteúdo dos arquivos. Tudo isso ele descarta quando um programa precisa de memória, mas dentro da VM quase nunca
          falta, e até lá o Windows conta como ocupado.
        </p>
        {#if sobra > 0 && resumo?.sugestao_bytes}
          <div class="sugestao" role="note">
            <p>
              <strong>O teto da VM é {bytes(resumo.limite_bytes)}, e o uso real chegou a {bytes(resumo.pico_bytes)}.</strong>
              O que sobra do teto vira cache. Com {bytes(resumo.sugestao_bytes)}, a VM devolve até {bytes(sobra)} ao Windows de vez.
              O novo limite vale no próximo início do WSL, ou na hora, se você reiniciar.
            </p>
            <button class="botao primario" onclick={() => (ajustando = true)}>Ajustar limite</button>
          </div>
        {/if}
        <div class="devolver">
          <button class="botao" onclick={devolver} disabled={devolvendo}>{devolvendo ? 'Devolvendo' : 'Devolver cache agora'}</button>
          <span role="status">
            {devolucao
              ? resultado(devolucao)
              : 'Esvazia o cache da VM e mede o efeito 15 s e 2 min depois. Se algo continuar lendo muitos arquivos, parte dele volta.'}
          </span>
        </div>
      </section>
    {/if}

    <ProjetosDocker />

    <section class="cartao largo" aria-labelledby="titulo-distros">
      <div class="cabeca">
        <h2 id="titulo-distros">Por distro</h2>
        <span>Soma dos processos de cada uma; memória compartilhada entra uma vez por processo, e o total pode passar do uso real da VM</span>
      </div>
      <table>
        <thead>
          <tr>
            <ColunaOrdenavel {ordem} coluna="nome" titulo="Distro" />
            <ColunaOrdenavel {ordem} coluna="processos" titulo="Processos" numerica />
            <ColunaOrdenavel {ordem} coluna="cpu" titulo="CPU" numerica />
            <ColunaOrdenavel {ordem} coluna="mem" titulo="Memória (soma do RSS)" numerica />
            <ColunaOrdenavel {ordem} coluna="agenteRss" titulo="Agente: memória" numerica />
            <ColunaOrdenavel {ordem} coluna="coleta" titulo="Agente: leitura" numerica />
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
  .sugestao {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    padding: 10px 12px;
    border: 1px solid var(--borda);
    border-left: 3px solid var(--destaque);
    border-radius: var(--raio);
    background: var(--selecionado);
    font-size: 13px;
  }

  .sugestao p {
    flex: 1;
    min-width: 240px;
    margin: 0;
  }

  .devolver {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    color: var(--texto-2);
    font-size: 12px;
  }

  .devolver span {
    flex: 1;
    min-width: 240px;
  }

  .nota {
    margin: 0;
    color: var(--texto-2);
    font-size: 12px;
    max-width: 90ch;
  }

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
