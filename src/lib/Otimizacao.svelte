<script lang="ts">
  // Plano de otimização: o app monta a partir do que mediu, a pessoa marca o que fazer, e só então
  // ele executa, um item de cada vez. O que mora fora do app vira passo para a pessoa fazer.
  import { api } from './api';
  import type { ItemOtimizacao, PlanoOtimizacao, RiscoOtimizacao } from './tipos';

  let { aberto = $bindable(false), onaplicou }: { aberto: boolean; onaplicou?: () => void } = $props();

  type Situacao = { estado: 'esperando' | 'rodando' | 'feito' | 'falhou'; texto: string };

  const RISCOS: Record<RiscoOtimizacao, string> = {
    seguro: 'Seguro',
    muda_comportamento: 'Muda o comportamento',
    irreversivel: 'Não tem volta',
  };

  let dialogo: HTMLDialogElement;
  let fase = $state<'montando' | 'plano' | 'executando' | 'pronto' | 'erro'>('montando');
  let plano = $state<PlanoOtimizacao | null>(null);
  let erro = $state('');
  let marcados = $state<Record<string, boolean>>({});
  let situacao = $state<Record<string, Situacao>>({});

  const executaveis = $derived(plano?.itens.filter((i) => i.acao.tipo !== 'voce_faz') ?? []);
  const voceFaz = $derived(plano?.itens.filter((i) => i.acao.tipo === 'voce_faz') ?? []);
  const escolhidos = $derived(executaveis.filter((i) => marcados[i.id]));
  const semVolta = $derived(escolhidos.filter((i) => i.risco === 'irreversivel'));

  const data = new Intl.DateTimeFormat('pt-BR', { day: '2-digit', month: '2-digit' });
  const gb = (b: number) => (b / 1024 ** 3).toFixed(1).replace('.', ',');

  async function montar() {
    fase = 'montando';
    situacao = {};
    try {
      plano = await api.sugerirOtimizacao();
      marcados = Object.fromEntries(plano.itens.map((i) => [i.id, i.marcado]));
      fase = 'plano';
    } catch (e) {
      erro = String(e);
      fase = 'erro';
    }
  }

  $effect(() => {
    if (aberto && !dialogo.open) {
      dialogo.showModal();
      montar();
    } else if (!aberto && dialogo.open) {
      dialogo.close();
    }
  });

  async function executar(i: ItemOtimizacao): Promise<string> {
    const a = i.acao;
    if (a.tipo === 'limpar_docker') return `${gb(await api.limparDocker(a.qual, a.volumes))} GB liberados`;
    if (a.tipo === 'limitar_cpu') {
      await api.limitarCpuConteiner(a.id, a.nome, a.nucleos);
      return `limitado a ${String(a.nucleos).replace('.', ',')} núcleos`;
    }
    return '';
  }

  async function realizar() {
    const fila = escolhidos;
    fase = 'executando';
    situacao = Object.fromEntries(fila.map((i) => [i.id, { estado: 'esperando', texto: 'na fila' }]));
    let feitos = 0;
    const resumo: string[] = [];
    // Um de cada vez: duas limpezas do Docker juntas disputam o mesmo disco.
    for (const i of fila) {
      situacao[i.id] = { estado: 'rodando', texto: 'em andamento' };
      try {
        const texto = await executar(i);
        situacao[i.id] = { estado: 'feito', texto: texto || 'feito' };
        resumo.push(texto ? `${i.titulo} (${texto})` : i.titulo);
        feitos++;
      } catch (e) {
        situacao[i.id] = { estado: 'falhou', texto: String(e) };
      }
    }
    if (feitos) {
      try {
        await api.registrarOtimizacao(`Plano aplicado: ${feitos} de ${fila.length} item(ns). ${resumo.join('; ')}.`);
      } catch {
        // Sem o registro, o plano continua aplicado; só a comparação de antes e depois fica sem marco.
      }
      onaplicou?.();
    }
    fase = 'pronto';
  }

  function fechar() {
    aberto = false;
  }
</script>

<dialog
  bind:this={dialogo}
  aria-labelledby="otimizacao-titulo"
  aria-busy={fase === 'montando' || fase === 'executando'}
  onclose={() => (aberto = false)}
  oncancel={(e) => fase === 'executando' && e.preventDefault()}
>
  <header>
    <h2 id="otimizacao-titulo">Plano de otimização</h2>
    {#if plano && fase !== 'montando'}
      <p class="explica">
        Montado com as ocorrências desde {data.format(plano.desde_ms)}: {plano.travas} trava(s) e {plano.sobrecargas} sobrecarga(s).
        Nada muda até você escolher e confirmar.
      </p>
    {/if}
  </header>

  <div class="corpo">
    {#if fase === 'montando'}
      <p role="status">Lendo as ocorrências e o espaço do Docker. Com muitos volumes, isso leva alguns segundos.</p>
    {:else if fase === 'erro'}
      <p role="alert">Não foi possível montar o plano: {erro}</p>
    {:else if plano}
      {#if plano.quem_pesou.length}
        <section aria-labelledby="pesou-titulo">
          <h3 id="pesou-titulo">O que mais pesou antes delas</h3>
          <ul class="pesou">
            {#each plano.quem_pesou as p (p.nome)}
              <li>{p.nome}: em {p.ocorrencias}</li>
            {/each}
          </ul>
        </section>
      {/if}

      {#each plano.avisos as a (a)}
        <p class="aviso">{a}</p>
      {/each}

      {#if executaveis.length}
        <section aria-labelledby="app-faz-titulo">
          <h3 id="app-faz-titulo">O app faz</h3>
          <ul class="itens">
            {#each executaveis as i (i.id)}
              {@const s = situacao[i.id]}
              <li class={i.risco}>
                <label>
                  <input type="checkbox" bind:checked={marcados[i.id]} disabled={fase !== 'plano'} aria-describedby="m-{i.id} e-{i.id}" />
                  <span class="titulo">{i.titulo}</span>
                  <span class="risco">{RISCOS[i.risco]}</span>
                </label>
                <p id="m-{i.id}">{i.motivo}</p>
                <p id="e-{i.id}" class="efeito">{i.efeito}</p>
                {#if i.detalhes.length}
                  <details>
                    <summary>Ver a lista ({i.detalhes.length})</summary>
                    <ul class="detalhes">
                      {#each i.detalhes as d (d)}<li>{d}</li>{/each}
                    </ul>
                  </details>
                {/if}
                {#if s}<p class="situacao {s.estado}" role="status">{s.estado === 'falhou' ? `Falhou: ${s.texto}` : s.texto}</p>{/if}
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if voceFaz.length}
        <section aria-labelledby="voce-faz-titulo">
          <h3 id="voce-faz-titulo">Você faz</h3>
          <p class="explica">Estes ficam fora do alcance do app. Os passos estão aqui para quando você quiser.</p>
          <ul class="itens">
            {#each voceFaz as i (i.id)}
              <li class={i.risco}>
                <span class="titulo">{i.titulo}</span>
                <p>{i.motivo}</p>
                <p class="efeito">{i.efeito}</p>
                {#if i.acao.tipo === 'voce_faz'}
                  <ol class="passos">
                    {#each i.acao.passos as p (p)}<li>{p}</li>{/each}
                  </ol>
                {/if}
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if !plano.itens.length}
        <p>Nada a sugerir agora: o Docker não tem o que limpar e nenhum container pesou de forma repetida antes das ocorrências.</p>
      {/if}
    {/if}
  </div>

  <footer>
    {#if fase === 'plano' && semVolta.length}
      <p class="aviso">Sem volta: {semVolta.map((i) => i.titulo.toLowerCase()).join('; ')}.</p>
    {/if}
    {#if fase === 'pronto'}
      <p role="status">Pronto. O que foi feito ficou registrado em Ocorrências, para comparar as travas de antes e de depois.</p>
    {/if}
    <div class="acoes">
      {#if fase === 'plano'}
        <button class="botao primario" onclick={realizar} disabled={!escolhidos.length}>
          Realizar {escolhidos.length} item(ns)
        </button>
      {/if}
      {#if fase === 'pronto' || fase === 'erro'}
        <button class="botao" onclick={montar}>Montar de novo</button>
      {/if}
      <button class="botao" onclick={fechar} disabled={fase === 'executando'}>Fechar</button>
    </div>
  </footer>
</dialog>

<style>
  dialog {
    width: min(720px, calc(100vw - 32px));
    max-height: calc(100vh - 48px);
    padding: 0;
    border: 1px solid var(--borda-forte);
    border-radius: 8px;
    background: var(--camada-forte);
    color: var(--texto);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.28);
  }

  dialog[open] {
    display: flex;
    flex-direction: column;
  }

  dialog::backdrop {
    background: rgba(0, 0, 0, 0.3);
  }

  header {
    padding: 20px 24px 8px;
  }

  h2 {
    margin: 0 0 6px;
    font-size: 20px;
    font-weight: 600;
  }

  h3 {
    margin: 14px 0 6px;
    font-size: 14px;
    font-weight: 600;
  }

  .corpo {
    flex: 1;
    overflow: auto;
    padding: 0 24px 8px;
    font-size: 13px;
  }

  .explica {
    margin: 0;
    color: var(--texto-2);
    font-size: 13px;
  }

  .pesou {
    margin: 0;
    padding-left: 18px;
  }

  .aviso {
    margin: 8px 0;
    color: var(--perigo);
    font-size: 13px;
  }

  .itens {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .itens > li {
    padding: 10px 12px;
    border: 1px solid var(--borda);
    border-left: 3px solid var(--borda);
    border-radius: 6px;
  }

  .itens > li.muda_comportamento {
    border-left-color: var(--destaque);
  }

  .itens > li.irreversivel {
    border-left-color: var(--perigo);
  }

  label {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .titulo {
    font-weight: 600;
  }

  .risco {
    margin-left: auto;
    color: var(--texto-2);
    font-size: 12px;
    white-space: nowrap;
  }

  .itens p {
    margin: 6px 0 0;
  }

  .efeito {
    color: var(--texto-2);
  }

  details {
    margin-top: 6px;
  }

  .detalhes {
    max-height: 160px;
    overflow: auto;
    margin: 6px 0 0;
    padding-left: 18px;
    font-family: var(--mono, monospace);
    font-size: 12px;
  }

  .passos {
    margin: 6px 0 0;
    padding-left: 18px;
  }

  .situacao.feito {
    color: var(--texto);
    font-weight: 600;
  }

  .situacao.falhou {
    color: var(--perigo);
  }

  .situacao.esperando,
  .situacao.rodando {
    color: var(--texto-2);
  }

  footer {
    padding: 8px 24px 20px;
    border-top: 1px solid var(--borda);
    font-size: 13px;
  }

  .acoes {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
  }
</style>
