<script lang="ts">
  // Só o que é RAM: da VM do WSL e do Windows. Espaço em disco fica na aba Disco.
  import { onMount } from 'svelte';
  import { api } from './api';
  import AvisoMemoria from './AvisoMemoria.svelte';
  import BarraMemoria from './BarraMemoria.svelte';
  import ColunaOrdenavel from './ColunaOrdenavel.svelte';
  import { estado } from './estado.svelte';
  import { bytes } from './formato';
  import Grafico from './Grafico.svelte';
  import LimiteMemoria from './LimiteMemoria.svelte';
  import LimitesMemoria from './LimitesMemoria.svelte';
  import { comparar, Ordenacao } from './ordenacao.svelte';
  import ProjetosDocker from './ProjetosDocker.svelte';
  import type { Devolucao, ResumoMemoria } from './tipos';

  type Coluna = 'nome' | 'mem' | 'agenteRss';
  const ordem = new Ordenacao<Coluna>('memoria-por-distro', 'mem', (c) => c === 'nome');

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
        mem: a.processos.reduce((s, p) => s + p.rss, 0),
        agenteRss: a.agente.rss,
      })),
      (d, c) => d[c],
      (a, b) => comparar(a.nome, b.nome),
    ),
  );
</script>

<AvisoMemoria onajustar={() => (ajustando = true)} />

{#if !vm}
  <div class="vazio">
    <strong>Nenhuma distro em execução</strong>
    <span>A VM do WSL só pode ser medida por dentro de uma distro ligada.</span>
    <button class="botao" onclick={() => (estado.pagina = 'distros')}>Ver distros</button>
  </div>
  <!-- Os limites valem no próximo início, então fazem sentido justo com o WSL desligado. -->
  <div class="rolagem">
    <LimitesMemoria {resumo} onajustar={() => (ajustando = true)} />
  </div>
{:else}
  <div class="rolagem">
    <section class="cartao largo" aria-labelledby="titulo-mem">
      <div class="cabeca">
        <h2 id="titulo-mem">Uso de RAM da VM</h2>
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
        <div><dt>Cache de arquivos</dt><dd>{bytes(cache)}</dd></div>
        {#if windows?.wsl_bytes != null}
          <div><dt>Na RAM do Windows</dt><dd>{bytes(windows.wsl_bytes)}</dd></div>
        {/if}
        <div><dt>Swap em uso</dt><dd>{bytes(swapUsada)} de {bytes(vm.swap_total)}</dd></div>
      </dl>
    </section>

    <LimitesMemoria {resumo} onajustar={() => (ajustando = true)} />

    {#if windows}
      <section class="cartao largo" aria-labelledby="titulo-onde">
        <div class="cabeca">
          <h2 id="titulo-onde">Para onde vai a RAM</h2>
          <span>Memória comprometida do Windows: {bytes(windows.commit_bytes)} de {bytes(windows.commit_limite_bytes)}, contando o que pode ir para o arquivo de paginação</span>
        </div>
        <BarraMemoria
          titulo="RAM do Windows: {bytes(windows.total_bytes)}"
          partes={[
            { rotulo: 'VM do WSL', bytes: windows.wsl_bytes ?? 0, cor: 'var(--mem)' },
            { rotulo: 'Outros programas', bytes: usadaWindows - (windows.wsl_bytes ?? 0), cor: 'var(--cpu)' },
            { rotulo: 'Disponível', bytes: windows.disponivel_bytes, cor: 'var(--borda-forte)' },
          ]}
        />
        <BarraMemoria
          titulo="RAM dentro da VM do WSL: {bytes(vm.mem_total)}"
          partes={partes > 0
            ? [
                { rotulo: 'Programas e kernel', bytes: programas, cor: 'var(--mem)' },
                { rotulo: 'Cache: nomes e pastas', bytes: vm.mem_tabelas, cor: 'rgb(var(--calor))' },
                { rotulo: 'Cache: blocos lidos', bytes: vm.mem_blocos, cor: 'rgba(var(--calor), 0.65)' },
                { rotulo: 'Cache: conteúdo de arquivos', bytes: vm.mem_arquivos, cor: 'rgba(var(--calor), 0.4)' },
                { rotulo: 'Livre', bytes: vm.mem_livre, cor: 'var(--borda-forte)' },
              ]
            : [
                { rotulo: 'Programas e kernel', bytes: programas, cor: 'var(--mem)' },
                { rotulo: 'Cache de arquivos', bytes: cache, cor: 'rgb(var(--calor))' },
                { rotulo: 'Livre', bytes: vm.mem_livre, cor: 'var(--borda-forte)' },
              ]}
        />
        <p class="nota">
          Tudo nesta barra é RAM. As três partes de cache são cópias do que o Linux já leu, para não ler de novo: os nomes e
          as pastas de cada arquivo percorrido, pedaços lidos do disco e o conteúdo de arquivos abertos. Nada disso ocupa
          espaço em disco. O Linux descarta o cache quando um programa precisa de memória, mas dentro da VM quase nunca
          falta, e até lá o Windows conta como ocupado.
        </p>
        {#if sobra > 0 && resumo?.sugestao_bytes}
          <div class="sugestao" role="note">
            <p>
              <strong>O teto da VM é {bytes(resumo.limite_bytes)}, e o uso real chegou a {bytes(resumo.pico_bytes)}.</strong>
              O que sobra do teto vira cache de arquivos. Com {bytes(resumo.sugestao_bytes)}, a VM devolve até {bytes(sobra)} ao Windows de vez.
              O novo limite vale no próximo início do WSL, ou na hora, se você reiniciar.
            </p>
            <button class="botao primario" onclick={() => (ajustando = true)}>Ajustar limite</button>
          </div>
        {/if}
        <div class="devolver">
          <button class="botao" onclick={devolver} disabled={devolvendo}>{devolvendo ? 'Devolvendo' : 'Devolver o cache de arquivos agora'}</button>
          <span role="status">
            {devolucao
              ? resultado(devolucao)
              : 'Esvazia o cache de arquivos da VM, que está na RAM, e mede o efeito 15 s e 2 min depois. Nada se apaga do disco. Se algo continuar lendo muitos arquivos, parte dele volta.'}
          </span>
        </div>
      </section>
    {/if}

    <ProjetosDocker />

    <section class="cartao largo" aria-labelledby="titulo-distros">
      <div class="cabeca">
        <h2 id="titulo-distros">Por distro</h2>
        <span>Soma da RAM dos processos de cada uma; memória compartilhada entra uma vez por processo, e o total pode passar do uso real da VM</span>
      </div>
      <table>
        <thead>
          <tr>
            <ColunaOrdenavel {ordem} coluna="nome" titulo="Distro" />
            <ColunaOrdenavel {ordem} coluna="mem" titulo="RAM (soma do RSS)" numerica />
            <ColunaOrdenavel {ordem} coluna="agenteRss" titulo="Agente: RAM" numerica />
          </tr>
        </thead>
        <tbody>
          {#each porDistro as d (d.nome)}
            <tr>
              <th scope="row">{d.nome}</th>
              <td class="numero">{bytes(d.mem)}</td>
              <td class="numero">{bytes(d.agenteRss)}</td>
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
