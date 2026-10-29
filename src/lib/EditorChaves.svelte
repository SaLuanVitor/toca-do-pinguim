<script lang="ts">
  // Editor de um arquivo INI do WSL a partir de um catálogo de chaves. Campo vazio é o
  // padrão do WSL: a chave sai do arquivo. Nada é gravado até "Salvar".
  import { onMount } from 'svelte';
  import { caminhoDoArquivo, caminhoParaArquivo, idChave, validar, type Chave } from './catalogoWslconfig';
  import { estado } from './estado.svelte';
  import type { ConteudoWslconfig, MudancaWslconfig } from './tipos';

  interface Props {
    catalogo: Chave[];
    grupos: readonly string[];
    filtro: string;
    /** Frase sobre quando a mudança vale, mostrada abaixo do caminho do arquivo. */
    quandoVale: string;
    ler: () => Promise<ConteudoWslconfig>;
    gravar: (mudancas: MudancaWslconfig[], reiniciar: boolean) => Promise<void>;
    reiniciar: { botao: string; titulo: string; texto: () => string; feito: string };
  }

  let { catalogo, grupos, filtro, quandoVale, ler, gravar, reiniciar }: Props = $props();

  let caminho = $state('');
  let existe = $state(true);
  let original = $state<Record<string, string>>({});
  let edicao = $state<Record<string, string>>({});
  let desconhecidas = $state<[string, string, string][]>([]);
  let salvando = $state(false);
  let gravado = $state('');

  const conhecidas = $derived(new Set(catalogo.map(idChave)));

  function paraTela(c: Chave, v: string) {
    return c.tipo === 'caminho' ? caminhoDoArquivo(v) : v;
  }

  async function carregar() {
    try {
      const r = await ler();
      caminho = r.caminho;
      existe = r.existe;
      const valores: Record<string, string> = {};
      for (const [secao, chave, valor] of r.valores) valores[idChave({ secao, chave })] = valor;
      original = Object.fromEntries(catalogo.map((c) => [idChave(c), paraTela(c, valores[idChave(c)] ?? '')]));
      edicao = { ...original };
      desconhecidas = r.valores.filter(([secao, chave]) => !conhecidas.has(idChave({ secao, chave })));
    } catch (e) {
      estado.avisar(`Não foi possível ler a configuração: ${e}`);
    }
  }

  const visiveis = $derived.by(() => {
    const t = filtro.trim().toLowerCase();
    const lista = t
      ? catalogo.filter((c) => `${c.chave} ${c.descricao} ${c.grupo}`.toLowerCase().includes(t))
      : catalogo;
    return grupos.map((g) => ({ grupo: g, chaves: lista.filter((c) => c.grupo === g) })).filter((g) => g.chaves.length);
  });

  const erros = $derived(
    Object.fromEntries(catalogo.map((c) => [idChave(c), validar(c, edicao[idChave(c)] ?? '')])),
  );

  const mudancas = $derived(
    catalogo.filter((c) => (edicao[idChave(c)] ?? '').trim() !== (original[idChave(c)] ?? '')),
  );
  const temErro = $derived(mudancas.some((c) => erros[idChave(c)]));

  function paraArquivo(c: Chave): MudancaWslconfig {
    const v = (edicao[idChave(c)] ?? '').trim();
    return { secao: c.secao, chave: c.chave, valor: v === '' ? null : c.tipo === 'caminho' ? caminhoParaArquivo(v) : v };
  }

  async function salvar(comReinicio: boolean) {
    salvando = true;
    gravado = '';
    const n = mudancas.length;
    const quantas = `${n} ${n === 1 ? 'alteração gravada' : 'alterações gravadas'}`;
    try {
      await gravar(mudancas.map(paraArquivo), comReinicio);
      // Não relê: ler o wsl.conf de uma distro recém-terminada a ligaria de novo.
      original = Object.fromEntries(catalogo.map((c) => [idChave(c), (edicao[idChave(c)] ?? '').trim()]));
      edicao = { ...original };
      existe = true;
      gravado = comReinicio ? `${quantas}; ${reiniciar.feito}` : `${quantas}. ${quandoVale}`;
    } catch (e) {
      estado.avisar(String(e));
    } finally {
      salvando = false;
    }
  }

  function salvarEReiniciar() {
    estado.pedido = {
      titulo: reiniciar.titulo,
      texto: reiniciar.texto(),
      acao: 'Salvar e reiniciar',
      executar: () => salvar(true),
    };
  }

  onMount(carregar);
</script>

<p class="arquivo">
  <code>{caminho}</code>{existe ? '' : ' (ainda não existe; é criado ao salvar)'}. {quandoVale} Campo vazio usa
  o padrão do WSL.
</p>

<div class="rolagem">
  {#each visiveis as g (g.grupo)}
    <section aria-labelledby="grupo-{g.grupo}">
      <h2 id="grupo-{g.grupo}">{g.grupo}</h2>
      {#each g.chaves as c (idChave(c))}
        {@const id = idChave(c)}
        {@const erro = erros[id]}
        <div class="linha" class:alterada={(edicao[id] ?? '').trim() !== (original[id] ?? '')}>
          <div class="nome">
            <label for={id}><code>{c.chave}</code></label>
            {#if c.win11}<span class="selo">Windows 11</span>{/if}
            <p id="{id}-desc">{c.descricao}</p>
          </div>
          <div class="controle">
            {#if c.tipo === 'bool'}
              <select {id} bind:value={edicao[id]} aria-describedby="{id}-desc">
                <option value="">Padrão ({c.padrao})</option>
                <option value="true">Ligado</option>
                <option value="false">Desligado</option>
              </select>
            {:else if c.tipo === 'escolha'}
              <select {id} bind:value={edicao[id]} aria-describedby="{id}-desc">
                <option value="">Padrão ({c.padrao})</option>
                {#each c.opcoes ?? [] as o (o)}<option value={o}>{o}</option>{/each}
              </select>
            {:else}
              <input
                {id}
                type="text"
                spellcheck="false"
                placeholder="Padrão: {c.padrao}"
                bind:value={edicao[id]}
                aria-invalid={erro ? 'true' : undefined}
                aria-describedby="{id}-desc{erro ? ` ${id}-erro` : ''}"
              />
            {/if}
            {#if erro}<span class="erro" id="{id}-erro">{erro}</span>{/if}
          </div>
        </div>
      {/each}
    </section>
  {/each}

  {#if visiveis.length === 0}
    <div class="vazio"><strong>Nenhuma configuração com "{filtro}"</strong></div>
  {/if}

  {#if desconhecidas.length && !filtro}
    <section aria-labelledby="grupo-outras">
      <h2 id="grupo-outras">Outras chaves no arquivo</h2>
      <p class="arquivo">O app não conhece estas chaves e as mantém como estão.</p>
      {#each desconhecidas as [secao, chave, valor] (`${secao}.${chave}`)}
        <div class="linha"><code>[{secao}] {chave}={valor}</code></div>
      {/each}
    </section>
  {/if}
</div>

{#if mudancas.length || gravado}
  <footer class="rodape" role="status">
    {#if mudancas.length}
      <span>{mudancas.length} {mudancas.length === 1 ? 'alteração' : 'alterações'} sem salvar</span>
      <button class="botao" onclick={() => (edicao = { ...original })} disabled={salvando}>Descartar</button>
      <button class="botao" onclick={() => salvar(false)} disabled={salvando || temErro}>Salvar</button>
      <button class="botao primario" onclick={salvarEReiniciar} disabled={salvando || temErro}>
        {reiniciar.botao}
      </button>
    {:else}
      <span>{gravado}</span>
      <button class="botao sutil" onclick={() => (gravado = '')}>Fechar</button>
    {/if}
  </footer>
{/if}

<style>
  .arquivo {
    margin: 0 20px 8px;
    color: var(--texto-2);
    font-size: 13px;
  }

  .rolagem {
    flex: 1;
    overflow: auto;
    padding: 0 20px 20px;
  }

  h2 {
    margin: 18px 0 4px;
    font-size: 14px;
    font-weight: 600;
  }

  .linha {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(220px, 300px);
    gap: 16px;
    align-items: start;
    padding: 10px;
    border-bottom: 1px solid var(--borda);
    border-left: 3px solid transparent;
  }

  .linha.alterada {
    border-left-color: var(--destaque);
    background: var(--selecionado);
  }

  .nome p {
    margin: 2px 0 0;
    color: var(--texto-2);
    font-size: 12px;
  }

  .selo {
    margin-left: 6px;
    padding: 1px 6px;
    border: 1px solid var(--borda-forte);
    border-radius: 10px;
    color: var(--texto-2);
    font-size: 11px;
  }

  .controle {
    display: grid;
    gap: 4px;
  }

  .controle input,
  .controle select {
    height: 30px;
    padding: 0 8px;
    border: 1px solid var(--borda);
    border-radius: var(--raio);
    background: var(--camada-forte);
  }

  .controle input[aria-invalid='true'] {
    border-color: var(--perigo);
  }

  .erro {
    color: var(--perigo);
    font-size: 12px;
  }

  .rodape {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 10px 20px;
    border-top: 1px solid var(--borda);
    background: var(--camada-forte);
    font-size: 13px;
  }

  .rodape span {
    margin-right: auto;
  }

  @media (max-width: 640px) {
    .linha {
      grid-template-columns: 1fr;
      gap: 8px;
    }
  }
</style>
