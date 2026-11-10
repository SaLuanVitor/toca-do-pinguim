<script lang="ts">
  // Cada trava, sobrecarga, Docker sem resposta ou devolução de cache que falhou, com a carga dos
  // 10 min antes, os picos e como terminou. Serve para comparar uma trava com a outra e para levar
  // o caso adiante: "Copiar" põe o registro em Markdown na área de transferência.
  import { onMount } from 'svelte';
  import { api } from './api';
  import { estado } from './estado.svelte';
  import Otimizacao from './Otimizacao.svelte';
  import type { Carga, Ocorrencia, TipoOcorrencia } from './tipos';

  const ROTULOS: Record<TipoOcorrencia, string> = {
    trava: 'WSL travado',
    sobrecarga: 'VM sobrecarregada',
    docker: 'Docker sem resposta',
    devolucao: 'Devolução de cache falhou',
    otimizacao: 'Otimização aplicada',
  };
  const DIA_MS = 24 * 3600_000;

  let otimizando = $state(false);

  // A busca (Ctrl+K) pede o plano de fora da página.
  $effect(() => {
    if (estado.sugerirOtimizacao) {
      estado.sugerirOtimizacao = false;
      otimizando = true;
    }
  });
  /** O mesmo limiar em que o app deixa de devolver o cache sozinho. */
  const DISCO_CHEIO = 5;

  let lista = $state<Ocorrencia[]>([]);
  let carregou = $state(false);
  let erro = $state<string | null>(null);

  async function ler() {
    try {
      lista = await api.listarOcorrencias();
      erro = null;
    } catch (e) {
      erro = String(e);
    } finally {
      carregou = true;
    }
  }

  onMount(() => {
    ler();
    const t = setInterval(ler, 30_000);
    return () => clearInterval(t);
  });

  const data = new Intl.DateTimeFormat('pt-BR', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' });
  const hora = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' });
  const numero = (v: number) => v.toFixed(1).replace('.', ',');

  function duracao(o: Ocorrencia): string {
    if (o.fim_ms === null) return 'em andamento';
    const s = Math.round((o.fim_ms - o.inicio_ms) / 1000);
    if (s < 1) return '';
    if (s < 60) return `${s} s`;
    const m = Math.floor(s / 60);
    return m < 60 ? `${m} min ${s % 60} s` : `${Math.floor(m / 60)} h ${m % 60} min`;
  }

  /** Quem pesou nos 10 min antes, pelo nome, com o pico de CPU e de disco de cada um. */
  function quemPesou(antes: Carga[]): string[] {
    const picos = new Map<string, { cpu: number; disco: number }>();
    for (const c of antes) {
      for (const p of c.pesados) {
        const chave = `${p.nome} em ${p.distro}`;
        const atual = picos.get(chave) ?? { cpu: 0, disco: 0 };
        picos.set(chave, { cpu: Math.max(atual.cpu, p.cpu_nucleos), disco: Math.max(atual.disco, p.disco_bps) });
      }
    }
    return [...picos.entries()]
      .sort((a, b) => b[1].disco - a[1].disco || b[1].cpu - a[1].cpu)
      .slice(0, 4)
      .map(([nome, p]) => {
        const partes: string[] = [];
        if (p.cpu >= 0.5) partes.push(`até ${numero(p.cpu)} núcleos`);
        if (p.disco >= 1024 ** 2) partes.push(`até ${numero(p.disco / 1024 ** 2)} MB/s de disco`);
        return partes.length ? `${nome} (${partes.join(', ')})` : nome;
      });
  }

  const travas = $derived(lista.filter((o) => o.tipo === 'trava'));
  const travasComDisco = $derived(travas.filter((o) => o.antes.some((c) => c.disco_full >= DISCO_CHEIO)).length);

  /** Travas desde o último plano aplicado contra as do mesmo número de dias antes dele. */
  const comparacao = $derived.by(() => {
    const marco = lista.find((o) => o.tipo === 'otimizacao');
    if (!marco) return null;
    const dias = Math.max(1, Math.ceil((Date.now() - marco.inicio_ms) / DIA_MS));
    const depois = travas.filter((o) => o.inicio_ms >= marco.inicio_ms).length;
    const antes = travas.filter((o) => o.inicio_ms < marco.inicio_ms && o.inicio_ms >= marco.inicio_ms - dias * DIA_MS).length;
    return { quando: marco.inicio_ms, dias, depois, antes };
  });

  function relatorio(o: Ocorrencia): string {
    if (o.tipo === 'otimizacao') return `### ${ROTULOS.otimizacao}: ${data.format(o.inicio_ms)}\n\n${o.titulo}`;
    const linhas = [
      `### ${ROTULOS[o.tipo] ?? o.tipo}: ${data.format(o.inicio_ms)}`,
      '',
      o.titulo,
      '',
      `- Duração: ${duracao(o) || 'pontual'}`,
      `- Saída: ${o.saida ?? 'ainda aberta'}`,
      `- Pico de espera por disco (full, média de 60 s): ${numero(o.pico_disco)}%`,
      `- Pico de CPU da VM: ${numero(o.pico_cpu)}%`,
    ];
    const pesou = quemPesou(o.antes);
    if (pesou.length) linhas.push(`- Quem pesou nos 10 min antes: ${pesou.join('; ')}`);
    if (o.antes.length) {
      linhas.push('', '| Hora | Disco (full) | RAM (full) | CPU |', '| --- | --- | --- | --- |');
      for (const c of o.antes) linhas.push(`| ${hora.format(c.quando_ms)} | ${numero(c.disco_full)}% | ${numero(c.memoria_full)}% | ${numero(c.cpu_vm)}% |`);
    }
    return linhas.join('\n');
  }

  async function copiar(texto: string) {
    try {
      await navigator.clipboard.writeText(texto);
      estado.avisar('Relatório copiado.');
    } catch (e) {
      estado.avisar(`Não foi possível copiar: ${e}`);
    }
  }

  function copiarTudo() {
    copiar([`## Ocorrências do WSL (${lista.length})`, ...lista.map(relatorio)].join('\n\n'));
  }

  /** Linha da série de 0 a 100 dentro de 200 x 40. */
  function linha(antes: Carga[], valor: (c: Carga) => number): string {
    if (antes.length < 2) return '';
    return antes.map((c, i) => `${((i / (antes.length - 1)) * 200).toFixed(1)},${(40 - (Math.min(valor(c), 100) / 100) * 40).toFixed(1)}`).join(' ');
  }
</script>

<header class="barra">
  <h1>Ocorrências</h1>
  <button class="botao primario" onclick={() => (otimizando = true)}>Sugerir otimização</button>
  {#if lista.length}
    <button class="botao" onclick={copiarTudo}>Copiar tudo</button>
  {/if}
</header>

<Otimizacao bind:aberto={otimizando} onaplicou={ler} />

<div class="rolagem">
  {#if erro}
    <p class="nota">Não foi possível ler as ocorrências: {erro}</p>
  {:else if carregou && lista.length === 0}
    <div class="vazio">
      <strong>Nenhuma ocorrência registrada</strong>
      <span>
        Quando o WSL travar, a VM ficar sobrecarregada, o Docker parar de responder ou a devolução de cache falhar, o app
        registra aqui o que aconteceu, a carga dos 10 min antes e como terminou.
      </span>
    </div>
  {:else if travas.length}
    <p class="resumo">
      Travas com espera por disco acima de {DISCO_CHEIO}% nos 10 min antes: {travasComDisco} de {travas.length}.
    </p>
  {/if}
  {#if comparacao}
    <p class="resumo">
      Desde a última otimização ({data.format(comparacao.quando)}): {comparacao.depois} trava(s) em {comparacao.dias} dia(s). No mesmo número de dias antes dela:
      {comparacao.antes}.
    </p>
  {/if}

  <ol>
    {#each lista as o (`${o.tipo}|${o.inicio_ms}`)}
      {@const pesou = quemPesou(o.antes)}
      <li class:aberta={o.fim_ms === null} class:grave={o.tipo === 'trava'}>
        <div class="topo">
          <span class="tipo">{ROTULOS[o.tipo] ?? o.tipo}</span>
          <time datetime={new Date(o.inicio_ms).toISOString()}>{data.format(o.inicio_ms)}</time>
          {#if duracao(o)}<span class="duracao">{duracao(o)}</span>{/if}
          <button class="botao copiar" onclick={() => copiar(relatorio(o))}>Copiar</button>
        </div>
        <p class="titulo">{o.titulo}</p>
        {#if o.tipo !== 'otimizacao'}
        <dl>
          <div><dt>Saída</dt><dd>{o.saida ?? 'ainda aberta'}</dd></div>
          <div><dt>Pico de espera por disco</dt><dd>{numero(o.pico_disco)}%</dd></div>
          <div><dt>Pico de CPU da VM</dt><dd>{numero(o.pico_cpu)}%</dd></div>
          {#if pesou.length}<div><dt>Quem pesou antes</dt><dd>{pesou.join('; ')}</dd></div>{/if}
        </dl>
        {#if o.antes.length >= 2}
          <figure>
            <svg viewBox="0 0 200 40" preserveAspectRatio="none" aria-hidden="true">
              <polyline class="disco" points={linha(o.antes, (c) => c.disco_full)} />
              <polyline class="cpu" points={linha(o.antes, (c) => c.cpu_vm)} />
            </svg>
            <figcaption>
              {hora.format(o.antes[0].quando_ms)} a {hora.format(o.antes.at(-1)!.quando_ms)}:
              <span class="legenda disco">espera por disco</span> e <span class="legenda cpu">CPU da VM</span>, de 0 a 100%
            </figcaption>
          </figure>
        {/if}
        {/if}
      </li>
    {/each}
  </ol>
</div>

<style>
  .rolagem {
    flex: 1;
    overflow: auto;
    padding: 0 20px 20px;
  }

  .nota {
    margin: 14px 0 0;
    color: var(--texto-2);
    font-size: 13px;
  }

  .resumo {
    margin: 14px 0 4px;
    color: var(--texto-2);
    font-size: 13px;
  }

  ol {
    margin: 12px 0 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  li {
    padding: 12px 14px;
    border: 1px solid var(--borda);
    border-left: 3px solid var(--borda);
    border-radius: 6px;
    font-size: 13px;
  }

  li.grave {
    border-left-color: var(--perigo);
  }

  li.aberta {
    background: var(--camada-forte);
  }

  .topo {
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
  }

  .tipo {
    font-weight: 600;
  }

  time,
  .duracao {
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }

  .copiar {
    margin-left: auto;
    height: 26px;
  }

  .titulo {
    margin: 6px 0 8px;
  }

  dl {
    margin: 0;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 6px 16px;
  }

  dt {
    color: var(--texto-2);
    font-size: 12px;
  }

  dd {
    margin: 0;
  }

  figure {
    margin: 10px 0 0;
  }

  svg {
    width: 100%;
    height: 40px;
    border-bottom: 1px solid var(--borda);
  }

  polyline {
    fill: none;
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
  }

  .disco {
    stroke: var(--perigo);
    color: var(--perigo);
  }

  .cpu {
    stroke: var(--destaque);
    color: var(--destaque);
  }

  figcaption {
    margin-top: 4px;
    color: var(--texto-2);
    font-size: 12px;
  }

  .legenda {
    font-weight: 600;
  }
</style>
