<script lang="ts">
  import { desligarWsl, iniciar, ocupadas, reiniciar, terminar } from './acoes.svelte';
  import ColunaOrdenavel from './ColunaOrdenavel.svelte';
  import { estado } from './estado.svelte';
  import { bytes, pct } from './formato';
  import Icone from './Icone.svelte';
  import { comparar, Ordenacao } from './ordenacao.svelte';
  import type { Distro } from './tipos';

  type Coluna = 'nome' | 'situacao' | 'versao' | 'processos' | 'cpu' | 'mem';
  interface Linha {
    d: Distro;
    situacao: string;
    processos?: number;
    cpu?: number;
    mem?: number;
  }
  const ordem = new Ordenacao<Coluna>('distros', 'nome', (c) => c === 'nome' || c === 'situacao' || c === 'versao');

  /** Distro parada não tem processos medidos: fica no fim, não como zero. */
  function valor(l: Linha, c: Coluna): unknown {
    switch (c) {
      case 'nome': return l.d.nome;
      case 'versao': return l.d.versao;
      case 'cpu': return l.cpu === undefined ? null : Math.round(l.cpu * 10);
      default: return l[c];
    }
  }

  const linhas = $derived(
    ordem.ordenar(
      estado.distros.map((d): Linha => {
        const a = estado.amostras[d.nome];
        return {
          d,
          situacao: situacao(d),
          processos: a?.processos.length,
          cpu: a?.processos.reduce((s, p) => s + p.cpu_pct, 0),
          mem: a?.processos.reduce((s, p) => s + p.rss, 0),
        };
      }),
      valor,
      (a, b) => comparar(a.d.nome, b.d.nome),
    ),
  );
  const algumaRodando = $derived(estado.distros.some((d) => d.rodando));

  function situacao(d: Distro): string {
    if (ocupadas[d.nome]) return ocupadas[d.nome];
    if (!d.rodando) return 'Parada';
    return estado.ociosas.has(d.nome) ? 'Sem atividade' : 'Em execução';
  }
</script>

<header class="barra">
  <h1>Distros</h1>
  <div class="acoes">
    <button class="botao" disabled={!algumaRodando || !!ocupadas.WSL} onclick={desligarWsl} title={algumaRodando ? undefined : "Nenhuma distro está ligada"}>
      <Icone nome="energia" /> {ocupadas.WSL ?? 'Desligar WSL'}
    </button>
  </div>
</header>

{#if estado.distros.length === 0}
  <div class="vazio">
    <strong>Nenhuma distro instalada</strong>
    <span>Instale uma com <code>wsl --install</code> e ela aparece aqui.</span>
  </div>
{:else}
  <div class="rolagem">
    <table aria-label="Distros do WSL">
      <thead>
        <tr>
          <ColunaOrdenavel {ordem} coluna="nome" titulo="Nome" />
          <ColunaOrdenavel {ordem} coluna="situacao" titulo="Estado" />
          <ColunaOrdenavel {ordem} coluna="versao" titulo="Versão" numerica />
          <ColunaOrdenavel {ordem} coluna="processos" titulo="Processos" numerica />
          <ColunaOrdenavel {ordem} coluna="cpu" titulo="CPU" numerica />
          <ColunaOrdenavel
            {ordem}
            coluna="mem"
            titulo="Memória (soma do RSS)"
            numerica
            dica="Memória compartilhada entra uma vez por processo; o total pode passar do uso real da VM"
          />
          <th scope="col"><span class="visualmente-oculto">Ações</span></th>
        </tr>
      </thead>
      <tbody>
        {#each linhas as { d, processos, cpu, mem } (d.nome)}
          {@const ocupada = ocupadas[d.nome]}
          <tr>
            <th scope="row">
              {d.nome}
              {#if d.padrao}<span class="selo">Padrão</span>{/if}
            </th>
            <td>
              <span
                class="estado"
                class:ligada={d.rodando && !estado.ociosas.has(d.nome)}
                class:ociosa={d.rodando && estado.ociosas.has(d.nome)}
                aria-hidden="true"
              ></span>
              {situacao(d)}
            </td>
            <td class="numero">WSL {d.versao}</td>
            <td class="numero">{processos ?? ''}</td>
            <td class="numero">{cpu === undefined ? '' : pct(cpu)}</td>
            <td class="numero">{mem === undefined ? '' : bytes(mem)}</td>
            <td class="acoes-linha">
              {#if d.rodando}
                <button class="botao sutil" disabled={!!ocupada} onclick={() => reiniciar(d)} aria-label="Reiniciar {d.nome}">
                  <Icone nome="reiniciar" /> Reiniciar
                </button>
                <button class="botao sutil" disabled={!!ocupada} onclick={() => terminar(d)} aria-label="Terminar {d.nome}">
                  <Icone nome="parar" /> Terminar
                </button>
              {:else}
                <button class="botao sutil" disabled={!!ocupada} onclick={() => iniciar(d)} aria-label="Iniciar {d.nome}">
                  <Icone nome="play" /> Iniciar
                </button>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="nota">
      O app não segura nenhuma distro ligada. Quando a única sessão aberta numa distro é a
      do agente que lê os processos, ele sai, e o WSL a desliga no prazo normal de ociosidade.
    </p>
  </div>
{/if}

<style>
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

  table :global(th),
  td {
    height: 44px;
    padding: 0 10px;
    text-align: left;
    border-bottom: 1px solid var(--borda);
    white-space: nowrap;
  }

  thead :global(th) {
    height: 36px;
    color: var(--texto-2);
    font-weight: 400;
    font-size: 12px;
  }

  table :global(th.numero),
  td.numero {
    text-align: right;
  }

  tbody th {
    font-weight: 600;
  }

  tbody tr:hover {
    background: var(--hover);
  }

  .selo {
    margin-left: 8px;
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--selecionado);
    color: var(--destaque);
    font-size: 11px;
    font-weight: 600;
  }

  .estado {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 6px;
    border-radius: 50%;
    background: var(--texto-2);
    opacity: 0.5;
  }

  .estado.ligada {
    background: var(--ok);
    opacity: 1;
  }

  .estado.ociosa {
    background: rgb(var(--calor));
    opacity: 1;
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
