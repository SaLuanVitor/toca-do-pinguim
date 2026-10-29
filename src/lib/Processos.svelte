<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { encerrarProcesso } from './acoes.svelte';
  import ColunaOrdenavel from './ColunaOrdenavel.svelte';
  import { estado } from './estado.svelte';
  import { bytes, pct, rotuloEstado } from './formato';
  import Icone from './Icone.svelte';
  import { comparar, Ordenacao } from './ordenacao.svelte';
  import { lerPreferencia, salvarPreferencia } from './preferencias';
  import { SIGKILL, SIGTERM, type Processo } from './tipos';

  type Coluna = 'nome' | 'distro' | 'pid' | 'usuario' | 'estado' | 'cpu_pct' | 'rss' | 'cmdline';
  interface Linha {
    distro: string;
    p: Processo;
  }
  interface Grupo {
    distro: string;
    conectado: boolean;
    processos: Processo[];
    cpu: number;
    mem: number;
  }
  type Visual = { tipo: 'grupo'; g: Grupo } | { tipo: 'proc'; l: Linha };

  // Só as linhas na área visível vão para o DOM. Redesenhar a tabela inteira a cada
  // amostra custava 19 ms de layout por segundo com 180 processos, medido no WebView2.
  const ALTURA_LINHA = 30;
  const ALTURA_CABECALHO = 52;
  const MARGEM = 8;
  const DECIMO_DE_MB = 104_857.6;

  const ordem = new Ordenacao<Coluna>('processos', 'cpu_pct', (c) => c !== 'cpu_pct' && c !== 'rss');
  let agrupar = $state(lerPreferencia('processos.agrupar', true));
  let recolhidas = $state<Record<string, boolean>>(lerPreferencia('processos.recolhidas', {}));

  let busca = $state('');
  let selecao = $state<{ distro: string; pid: number } | null>(null);
  let menu = $state<{ x: number; y: number } | null>(null);
  let menuEl = $state<HTMLDivElement>();
  let rolagemEl = $state<HTMLDivElement>();
  let topo = $state(0);
  let alturaVisivel = $state(600);
  let ponteiroNaLista = $state(false);
  let focoNaLista = $state(false);

  /**
   * Enquanto o ponteiro ou o foco estão nas linhas, a ordem não muda: os valores seguem
   * atualizando, mas a linha não sai de baixo do clique nem da seta do teclado.
   * Processos novos entram no fim até a lista voltar a ordenar.
   */
  const congelada = $derived(ponteiroNaLista || focoNaLista);

  /**
   * Fora disso, as posições se rearranjam a cada 3 amostras, e os valores, a cada uma.
   * Medido com carga real (CI na docker-desktop), ordenar a cada amostra trocava cerca de
   * 25 linhas de lugar por segundo, quase todas as visíveis.
   */
  const AMOSTRAS_POR_REORDENACAO = 3;
  let ultimaOrdem = { assinatura: '', posicoes: new Map<string, number>(), quando: 0 };

  const colunas = $derived<{ id: Coluna; titulo: string; numerica?: boolean; classe: string }[]>([
    { id: 'nome', titulo: 'Nome', classe: 'c-nome' },
    ...(agrupar ? [] : [{ id: 'distro' as Coluna, titulo: 'Distro', classe: 'c-distro' }]),
    { id: 'pid', titulo: 'PID', numerica: true, classe: 'c-pid' },
    { id: 'usuario', titulo: 'Usuário', classe: 'c-usuario' },
    { id: 'estado', titulo: 'Status', classe: 'c-estado' },
    { id: 'cpu_pct', titulo: 'CPU', numerica: true, classe: 'c-cpu' },
    { id: 'rss', titulo: 'Memória', numerica: true, classe: 'c-rss' },
    { id: 'cmdline', titulo: 'Linha de comando', classe: 'c-cmd' },
  ]);

  /**
   * O valor ordenado é o que a tela mostra: 0,04% e 0,05% aparecem os dois como "0,0%" e,
   * comparados crus, trocariam de lugar a cada amostra.
   */
  function valor(l: Linha, c: Coluna): unknown {
    const p = l.p;
    switch (c) {
      case 'cpu_pct': return Math.round(p.cpu_pct * 10);
      case 'rss': return Math.round(p.rss / DECIMO_DE_MB);
      case 'estado': return rotuloEstado(p.estado);
      case 'distro': return l.distro;
      default: return p[c];
    }
  }

  function desempate(a: Linha, b: Linha): number {
    return comparar(a.p.nome, b.p.nome) || comparar(a.distro, b.distro) || a.p.pid - b.p.pid;
  }

  /** Grupos seguem a coluna quando ela tem total (CPU, memória) ou é o nome. */
  function valorGrupo(g: Grupo, c: Coluna): unknown {
    if (c === 'cpu_pct') return Math.round(g.cpu * 10);
    if (c === 'rss') return Math.round(g.mem / DECIMO_DE_MB);
    if (c === 'nome' || c === 'distro') return g.distro;
    return null;
  }

  function chave(l: Linha) {
    return `${l.distro}:${l.p.pid}`;
  }

  function chaveVisual(v: Visual) {
    return v.tipo === 'grupo' ? `grupo:${v.g.distro}` : chave(v.l);
  }

  const grupos = $derived.by((): Grupo[] => {
    const termo = busca.trim().toLowerCase();
    return estado.distros
      .filter((d) => d.rodando)
      .map((d) => {
        const amostra = estado.amostras[d.nome];
        const todos = amostra?.processos ?? [];
        return {
          distro: d.nome,
          conectado: !!amostra,
          processos: termo
            ? todos.filter(
                (p) =>
                  p.nome.toLowerCase().includes(termo) ||
                  String(p.pid) === termo ||
                  p.usuario.toLowerCase().includes(termo) ||
                  p.cmdline.toLowerCase().includes(termo),
              )
            : todos,
          cpu: todos.reduce((s, p) => s + p.cpu_pct, 0),
          mem: todos.reduce((s, p) => s + p.rss, 0),
        };
      });
  });

  // Mudar coluna, direção, agrupamento ou filtro é pedido explícito: reordena mesmo com
  // a lista congelada.
  const assinatura = $derived(`${ordem.coluna}|${ordem.crescente}|${agrupar}|${busca}`);

  function ordenarCom<T>(
    lista: T[],
    valorDe: (t: T, c: Coluna) => unknown,
    desempateDe: (a: T, b: T) => number,
    chaveDe: (t: T) => string,
    manter: boolean,
  ): T[] {
    if (!manter) return ordem.ordenar(lista, valorDe, desempateDe);
    const pos = ultimaOrdem.posicoes;
    const conhecidas = lista.filter((t) => pos.has(chaveDe(t))).sort((a, b) => pos.get(chaveDe(a))! - pos.get(chaveDe(b))!);
    const novas = ordem.ordenar(lista.filter((t) => !pos.has(chaveDe(t))), valorDe, desempateDe);
    return [...conhecidas, ...novas];
  }

  const montagem = $derived.by(() => {
    // `Date.now()` não é reativo: quem dispara o recálculo é a amostra nova; o relógio só
    // decide se esta é a vez de rearranjar.
    const recente = Date.now() - ultimaOrdem.quando < AMOSTRAS_POR_REORDENACAO * estado.intervalo;
    const manter = (congelada || recente) && ultimaOrdem.assinatura === assinatura;
    const ordenarLinhas = (ls: Linha[]) => ordenarCom(ls, valor, desempate, chave, manter);
    let visuais: Visual[];
    if (!agrupar) {
      const todas = grupos.flatMap((g) => g.processos.map((p) => ({ distro: g.distro, p })));
      visuais = ordenarLinhas(todas).map((l): Visual => ({ tipo: 'proc', l }));
    } else {
      const desempateGrupo = (a: Grupo, b: Grupo) => comparar(a.distro, b.distro);
      visuais = ordenarCom(grupos, valorGrupo, desempateGrupo, (g) => `grupo:${g.distro}`, manter).flatMap((g): Visual[] => [
        { tipo: 'grupo', g },
        ...(recolhidas[g.distro]
          ? []
          : ordenarLinhas(g.processos.map((p) => ({ distro: g.distro, p }))).map((l): Visual => ({ tipo: 'proc', l }))),
      ]);
    }
    return { visuais, rearranjou: !manter };
  });

  const visuais = $derived(montagem.visuais);

  $effect(() => {
    const { visuais, rearranjou } = montagem;
    ultimaOrdem = {
      assinatura,
      posicoes: new Map(visuais.map((v, i) => [chaveVisual(v), i])),
      quando: rearranjou ? Date.now() : ultimaOrdem.quando,
    };
  });

  const linhas = $derived(visuais.flatMap((v) => (v.tipo === 'proc' ? [v.l] : [])));

  const inicio = $derived(
    Math.max(0, Math.floor(Math.max(0, topo - ALTURA_CABECALHO) / ALTURA_LINHA) - MARGEM),
  );
  const fim = $derived(Math.min(visuais.length, Math.ceil((topo + alturaVisivel) / ALTURA_LINHA) + MARGEM));

  const selecionado = $derived(
    selecao ? linhas.find((l) => l.distro === selecao!.distro && l.p.pid === selecao!.pid) : undefined,
  );

  const vm = $derived(estado.vm);
  const memVm = $derived(vm && vm.mem_total ? (1 - vm.mem_disponivel / vm.mem_total) * 100 : 0);
  const protegido = $derived(!selecionado || selecionado.p.pid <= 1);
  const motivoBloqueio = $derived(
    !selecionado ? 'Selecione um processo na lista' : selecionado.p.pid <= 1 ? 'O init da distro só se encerra terminando a distro' : '',
  );

  function alternarAgrupar() {
    agrupar = !agrupar;
    salvarPreferencia('processos.agrupar', agrupar);
  }

  function alternarGrupo(distro: string) {
    recolhidas[distro] = !recolhidas[distro];
    salvarPreferencia('processos.recolhidas', $state.snapshot(recolhidas));
  }

  /** Rola até a linha antes de focar: fora da área visível ela não existe no DOM. */
  function trazerParaVista(l: Linha) {
    const el = rolagemEl;
    if (!el) return;
    const i = visuais.findIndex((v) => v.tipo === 'proc' && v.l.distro === l.distro && v.l.p.pid === l.p.pid);
    if (i < 0) return;
    const y = ALTURA_CABECALHO + i * ALTURA_LINHA;
    // Subindo, a linha de cima fica à vista junto: na primeira de um grupo, é o cabeçalho dele.
    if (y - ALTURA_CABECALHO < el.scrollTop) el.scrollTop = Math.max(0, y - ALTURA_CABECALHO - ALTURA_LINHA);
    else if (y + ALTURA_LINHA > el.scrollTop + el.clientHeight) el.scrollTop = y + ALTURA_LINHA - el.clientHeight;
    topo = el.scrollTop;
  }

  async function selecionar(l: Linha, focar = false) {
    selecao = { distro: l.distro, pid: l.p.pid };
    if (focar) {
      trazerParaVista(l);
      await tick();
      document.querySelector<HTMLElement>(`[data-chave="${CSS.escape(chave(l))}"]`)?.focus();
    }
  }

  async function sinal(s: number) {
    const alvo = selecionado;
    menu = null;
    if (alvo) await encerrarProcesso(alvo.distro, alvo.p.pid, alvo.p.nome, s);
  }

  // Seleção pedida pela busca: abre o grupo, limpa o filtro e seleciona quando a linha
  // existir (a amostra da distro pode ainda não ter chegado).
  $effect(() => {
    const f = estado.foco;
    if (!f) return;
    untrack(() => {
      if (recolhidas[f.distro]) alternarGrupo(f.distro);
      busca = '';
    });
    const l = linhas.find((x) => x.distro === f.distro && x.p.pid === f.pid);
    if (l) {
      estado.foco = null;
      selecionar(l, true);
    }
  });

  async function copiar() {
    const alvo = selecionado;
    menu = null;
    if (alvo) await navigator.clipboard.writeText(alvo.p.cmdline || alvo.p.nome);
  }

  async function abrirMenu(x: number, y: number) {
    menu = { x: Math.min(x, window.innerWidth - 230), y: Math.min(y, window.innerHeight - 140) };
    await tick();
    menuEl?.querySelector<HTMLElement>('[role="menuitem"]:not(:disabled)')?.focus();
  }

  function fecharMenu() {
    menu = null;
    if (selecionado) selecionar(selecionado, true);
  }

  function teclaTabela(e: KeyboardEvent) {
    if (!linhas.length) return;
    const i = selecionado ? linhas.indexOf(selecionado) : -1;
    const ir = (n: number) => {
      e.preventDefault();
      selecionar(linhas[Math.max(0, Math.min(linhas.length - 1, n))], true);
    };
    switch (e.key) {
      case 'ArrowDown': return ir(i + 1);
      case 'ArrowUp': return ir(i - 1);
      case 'PageDown': return ir(i + 15);
      case 'PageUp': return ir(i - 15);
      case 'Home': return ir(0);
      case 'End': return ir(linhas.length - 1);
      case 'Delete':
        e.preventDefault();
        sinal(e.shiftKey ? SIGKILL : SIGTERM);
        return;
      case 'ContextMenu':
      case 'F10': {
        if (e.key === 'F10' && !e.shiftKey) return;
        e.preventDefault();
        const linha = document.activeElement?.closest('tr');
        const r = linha?.getBoundingClientRect();
        if (r) abrirMenu(r.left + 40, r.bottom);
      }
    }
  }

  function teclaMenu(e: KeyboardEvent) {
    const itens = [...(menuEl?.querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled)') ?? [])];
    const i = itens.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'Escape' || e.key === 'Tab') {
      e.preventDefault();
      fecharMenu();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      itens[(i + 1) % itens.length]?.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      itens[(i - 1 + itens.length) % itens.length]?.focus();
    }
  }

  function teclaBusca(e: KeyboardEvent) {
    if (e.key === 'Escape' && busca) {
      e.preventDefault();
      busca = '';
    }
  }

  function calor(fracao: number): string {
    if (fracao <= 0.002) return '';
    return `background-color: rgba(var(--calor), ${(0.12 + Math.min(1, fracao) * 0.55).toFixed(3)})`;
  }
</script>

<svelte:window onclick={() => (menu = null)} onblur={() => (menu = null)} />

<header class="barra">
  <h1>Processos</h1>
  <label class="busca">
    <Icone nome="busca" />
    <span class="visualmente-oculto">Filtrar processos</span>
    <input
      type="search"
      placeholder="Nome, PID, usuário ou comando"
      bind:value={busca}
      onkeydown={teclaBusca}
      data-filtro
    />
  </label>
  <div class="acoes">
    <button class="botao" aria-pressed={agrupar} onclick={alternarAgrupar} title="Mostrar os processos separados por distro ou numa lista só">
      Agrupar por distro
    </button>
    <button class="botao" disabled={protegido} onclick={() => sinal(SIGTERM)} title={motivoBloqueio || 'Envia SIGTERM (Delete)'}>
      <Icone nome="parar" /> Finalizar tarefa
    </button>
    <button class="botao perigo" disabled={protegido} onclick={() => sinal(SIGKILL)} title={motivoBloqueio || 'Envia SIGKILL (Shift+Delete)'}>
      Forçar encerramento
    </button>
  </div>
  {#if congelada && grupos.length}
    <p class="pausa">Ordem pausada enquanto o ponteiro ou o foco estão na lista</p>
  {/if}
</header>

{#if grupos.length === 0}
  <div class="vazio">
    <strong>Nenhuma distro em execução</strong>
    <span>Os processos de uma distro só aparecem com ela ligada.</span>
    <button class="botao" onclick={() => (estado.pagina = 'distros')}>Ver distros</button>
  </div>
{:else}
  <div
    class="rolagem"
    bind:this={rolagemEl}
    bind:clientHeight={alturaVisivel}
    onscroll={(e) => (topo = e.currentTarget.scrollTop)}
  >
    <!-- As teclas chegam por borbulhamento das linhas, que são as focáveis. -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <table
      aria-label={agrupar ? 'Processos por distro' : 'Processos de todas as distros'}
      aria-rowcount={visuais.length + 1}
      onkeydown={teclaTabela}
    >
      <thead>
        <tr aria-rowindex={1}>
          {#each colunas as c (c.id)}
            <ColunaOrdenavel
              {ordem}
              coluna={c.id}
              titulo={c.titulo}
              numerica={c.numerica}
              classe={c.classe}
              total={vm && c.id === 'cpu_pct' ? pct(vm.cpu_vm_pct) : vm && c.id === 'rss' ? pct(memVm) : c.numerica ? '' : undefined}
            />
          {/each}
        </tr>
      </thead>
      <tbody
        onpointerenter={() => (ponteiroNaLista = true)}
        onpointerleave={() => (ponteiroNaLista = false)}
        onfocusin={() => (focoNaLista = true)}
        onfocusout={(e) => (focoNaLista = e.currentTarget.contains(e.relatedTarget as Node))}
      >
        {#if inicio > 0}
          <tr class="espaco" aria-hidden="true" style:height="{inicio * ALTURA_LINHA}px">
            <td colspan={colunas.length}></td>
          </tr>
        {/if}
        {#each visuais.slice(inicio, fim) as v, k (chaveVisual(v))}
          {#if v.tipo === 'grupo'}
            {@const g = v.g}
            <tr class="grupo" aria-rowindex={inicio + k + 2}>
              <th scope="row" colspan="4">
                <button aria-expanded={!recolhidas[g.distro]} onclick={() => alternarGrupo(g.distro)}>
                  <span class="seta" class:aberta={!recolhidas[g.distro]}><Icone nome="chevron" tamanho={12} /></span>
                  {g.distro}
                  <span class="contagem">
                    ({g.conectado ? g.processos.length : estado.ociosas.has(g.distro) ? 'sem atividade, deve desligar' : 'conectando'})
                  </span>
                </button>
              </th>
              <td class="numero">{g.conectado ? pct(g.cpu) : ''}</td>
              <td class="numero" title="Soma do RSS dos processos: memória compartilhada entra uma vez por processo">
                {g.conectado ? bytes(g.mem) : ''}
              </td>
              <td></td>
            </tr>
          {:else}
            {@const l = v.l}
            {@const p = l.p}
            {@const sel = selecao?.distro === l.distro && selecao.pid === p.pid}
            <tr
              data-chave={chave(l)}
              aria-rowindex={inicio + k + 2}
              tabindex={sel || (!selecionado && linhas[0]?.p === p) ? 0 : -1}
              aria-selected={sel}
              class:sel
              onclick={() => selecionar(l)}
              oncontextmenu={(e) => {
                e.preventDefault();
                e.stopPropagation();
                selecionar(l);
                abrirMenu(e.clientX, e.clientY);
              }}
            >
              <td class="nome" class:recuo={agrupar}>{p.nome}</td>
              {#if !agrupar}<td class="distro">{l.distro}</td>{/if}
              <td class="numero">{p.pid}</td>
              <td>{p.usuario}</td>
              <td class="status">{rotuloEstado(p.estado)}</td>
              <td class="numero" style={calor(p.cpu_pct / 25)}>{pct(p.cpu_pct)}</td>
              <td class="numero" style={calor(vm ? (p.rss / vm.mem_total) * 8 : 0)}>{bytes(p.rss)}</td>
              <td class="cmd" title={p.cmdline}>{p.cmdline}</td>
            </tr>
          {/if}
        {/each}
        {#if fim < visuais.length}
          <tr class="espaco" aria-hidden="true" style:height="{(visuais.length - fim) * ALTURA_LINHA}px">
            <td colspan={colunas.length}></td>
          </tr>
        {/if}
      </tbody>
    </table>
    {#if busca && linhas.length === 0}
      <p class="vazio">Nenhum processo corresponde a "{busca}".</p>
    {/if}
  </div>
{/if}

{#if menu}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    class="menu"
    role="menu"
    tabindex="-1"
    aria-label="Ações do processo"
    style:left="{menu.x}px"
    style:top="{menu.y}px"
    bind:this={menuEl}
    onkeydown={teclaMenu}
    onclick={(e) => e.stopPropagation()}
  >
    <button role="menuitem" disabled={protegido} onclick={() => sinal(SIGTERM)}>
      Finalizar tarefa <kbd>Delete</kbd>
    </button>
    <button role="menuitem" disabled={protegido} onclick={() => sinal(SIGKILL)}>
      Forçar encerramento <kbd>Shift+Delete</kbd>
    </button>
    <hr />
    <button role="menuitem" onclick={copiar}>Copiar linha de comando</button>
  </div>
{/if}

<style>
  .busca {
    display: flex;
    align-items: center;
    gap: 8px;
    width: min(340px, 100%);
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

  .pausa {
    flex-basis: 100%;
    margin: -4px 0 0;
    color: var(--texto-2);
    font-size: 12px;
  }

  .rolagem {
    flex: 1;
    overflow: auto;
    padding: 0 8px 8px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
    table-layout: fixed;
  }

  table :global(th),
  td {
    height: 30px;
    padding: 0 10px;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border-left: 1px solid var(--borda);
  }

  table :global(th:first-child),
  td:first-child {
    border-left: 0;
  }

  td.numero {
    text-align: right;
  }

  tr.espaco td {
    height: auto;
    padding: 0;
    border: 0;
  }

  thead :global(th) {
    position: sticky;
    top: 0;
    z-index: 1;
    height: 52px;
    padding: 0;
    vertical-align: bottom;
    font-weight: 400;
    background: var(--fundo);
    border-bottom: 1px solid var(--borda);
  }

  :global(:root.mica) thead :global(th) {
    background: color-mix(in srgb, var(--fundo) 92%, transparent);
  }

  thead :global(th.c-nome) { width: 22%; }
  thead :global(th.c-distro) { width: 120px; }
  thead :global(th.c-pid) { width: 72px; }
  thead :global(th.c-usuario) { width: 96px; }
  thead :global(th.c-estado) { width: 120px; }
  thead :global(th.c-cpu) { width: 80px; }
  thead :global(th.c-rss) { width: 104px; }

  thead :global(th button) {
    height: 52px;
  }

  tr.grupo th {
    padding: 0;
    font-weight: 600;
  }

  tr.grupo button {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 30px;
    padding: 0 6px;
    border: 0;
    background: transparent;
    font-weight: 600;
  }

  tr.grupo:hover,
  tbody tr:not(.grupo):hover {
    background: var(--hover);
  }

  .seta {
    display: inline-flex;
    transition: transform 0.12s;
  }

  .seta.aberta {
    transform: rotate(90deg);
  }

  .contagem {
    color: var(--texto-2);
    font-weight: 400;
  }

  td.nome.recuo {
    padding-left: 28px;
  }

  td.status,
  td.cmd,
  td.distro {
    color: var(--texto-2);
  }

  tr.sel,
  tr.sel:hover {
    background: var(--selecionado);
  }

  tr[tabindex]:focus-visible {
    outline-offset: -2px;
  }

  .menu {
    position: fixed;
    z-index: 10;
    min-width: 220px;
    padding: 4px;
    border: 1px solid var(--borda-forte);
    border-radius: 8px;
    background: var(--camada-forte);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.22);
  }

  .menu button {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    width: 100%;
    height: 32px;
    padding: 0 10px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    text-align: left;
  }

  .menu button:hover:not(:disabled),
  .menu button:focus-visible {
    background: var(--hover);
  }

  .menu button:disabled {
    opacity: 0.45;
  }

  .menu hr {
    margin: 4px 0;
    border: 0;
    border-top: 1px solid var(--borda);
  }

  kbd {
    color: var(--texto-2);
    font: inherit;
    font-size: 12px;
  }
</style>
