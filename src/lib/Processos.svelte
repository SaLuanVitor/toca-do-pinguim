<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { encerrarProcesso } from './acoes.svelte';
  import { estado } from './estado.svelte';
  import { bytes, pct, rotuloEstado } from './formato';
  import Icone from './Icone.svelte';
  import { SIGKILL, SIGTERM, type Processo } from './tipos';

  type Coluna = 'nome' | 'pid' | 'usuario' | 'estado' | 'cpu_pct' | 'rss' | 'cmdline';
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

  const COLUNAS: { id: Coluna; titulo: string; numerica?: boolean }[] = [
    { id: 'nome', titulo: 'Nome' },
    { id: 'pid', titulo: 'PID', numerica: true },
    { id: 'usuario', titulo: 'Usuário' },
    { id: 'estado', titulo: 'Status' },
    { id: 'cpu_pct', titulo: 'CPU', numerica: true },
    { id: 'rss', titulo: 'Memória', numerica: true },
    { id: 'cmdline', titulo: 'Linha de comando' },
  ];

  let busca = $state('');
  let coluna = $state<Coluna>('cpu_pct');
  let crescente = $state(false);
  let recolhidas = $state<Record<string, boolean>>({});
  let selecao = $state<{ distro: string; pid: number } | null>(null);
  let menu = $state<{ x: number; y: number } | null>(null);
  let menuEl = $state<HTMLDivElement>();
  let rolagemEl = $state<HTMLDivElement>();
  let topo = $state(0);
  let alturaVisivel = $state(600);

  function comparar(a: Processo, b: Processo): number {
    const va = a[coluna];
    const vb = b[coluna];
    const r = typeof va === 'number' && typeof vb === 'number'
      ? va - vb
      : String(va).localeCompare(String(vb), 'pt-BR', { sensitivity: 'base' });
    return (crescente ? r : -r) || a.pid - b.pid;
  }

  const grupos = $derived.by((): Grupo[] => {
    const termo = busca.trim().toLowerCase();
    return estado.distros
      .filter((d) => d.rodando)
      .map((d) => {
        const amostra = estado.amostras[d.nome];
        const todos = amostra?.processos ?? [];
        const processos = (termo
          ? todos.filter(
              (p) =>
                p.nome.toLowerCase().includes(termo) ||
                String(p.pid) === termo ||
                p.usuario.toLowerCase().includes(termo) ||
                p.cmdline.toLowerCase().includes(termo),
            )
          : [...todos]
        ).sort(comparar);
        return {
          distro: d.nome,
          conectado: !!amostra,
          processos,
          cpu: todos.reduce((s, p) => s + p.cpu_pct, 0),
          mem: todos.reduce((s, p) => s + p.rss, 0),
        };
      });
  });

  const visuais = $derived<Visual[]>(
    grupos.flatMap((g): Visual[] => [
      { tipo: 'grupo', g },
      ...(recolhidas[g.distro] ? [] : g.processos.map((p): Visual => ({ tipo: 'proc', l: { distro: g.distro, p } }))),
    ]),
  );

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

  function ordenar(c: Coluna) {
    if (coluna === c) crescente = !crescente;
    else {
      coluna = c;
      crescente = c !== 'cpu_pct' && c !== 'rss';
    }
  }

  function chave(l: Linha) {
    return `${l.distro}:${l.p.pid}`;
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
      recolhidas[f.distro] = false;
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
    <input type="search" placeholder="Nome, PID, usuário ou comando" bind:value={busca} />
  </label>
  <div class="acoes">
    <button class="botao" disabled={protegido} onclick={() => sinal(SIGTERM)} title="Envia SIGTERM (Delete)">
      <Icone nome="parar" /> Finalizar tarefa
    </button>
    <button class="botao perigo" disabled={protegido} onclick={() => sinal(SIGKILL)} title="Envia SIGKILL (Shift+Delete)">
      Forçar encerramento
    </button>
  </div>
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
    <table aria-label="Processos por distro" aria-rowcount={visuais.length + 1} onkeydown={teclaTabela}>
      <thead>
        <tr aria-rowindex={1}>
          {#each COLUNAS as c (c.id)}
            <th
              scope="col"
              class:numero={c.numerica}
              class:cmd={c.id === 'cmdline'}
              aria-sort={coluna === c.id ? (crescente ? 'ascending' : 'descending') : 'none'}
            >
              <button onclick={() => ordenar(c.id)}>
                {#if c.id === 'cpu_pct' && vm}
                  <span class="total">{pct(vm.cpu_vm_pct)}</span>
                {:else if c.id === 'rss' && vm}
                  <span class="total">{pct(memVm)}</span>
                {/if}
                <span class="titulo">{c.titulo}</span>
              </button>
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#if inicio > 0}
          <tr class="espaco" aria-hidden="true" style:height="{inicio * ALTURA_LINHA}px"><td colspan="7"></td></tr>
        {/if}
        {#each visuais.slice(inicio, fim) as v, k (v.tipo === 'grupo' ? `grupo:${v.g.distro}` : chave(v.l))}
          {#if v.tipo === 'grupo'}
            {@const g = v.g}
            <tr class="grupo" aria-rowindex={inicio + k + 2}>
              <th scope="row" colspan="4">
                <button
                  aria-expanded={!recolhidas[g.distro]}
                  onclick={() => (recolhidas[g.distro] = !recolhidas[g.distro])}
                >
                  <span class="seta" class:aberta={!recolhidas[g.distro]}><Icone nome="chevron" tamanho={12} /></span>
                  {g.distro}
                  <span class="contagem">
                    ({g.conectado ? g.processos.length : estado.ociosas.has(g.distro) ? 'sem atividade, deve desligar' : 'conectando'})
                  </span>
                </button>
              </th>
              <td class="numero">{g.conectado ? pct(g.cpu) : ''}</td>
              <td class="numero" title="Soma do RSS dos processos: memória compartilhada entra uma vez por processo">{g.conectado ? bytes(g.mem) : ''}</td>
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
              <td class="nome">{p.nome}</td>
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
            <td colspan="7"></td>
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

  th,
  td {
    padding: 0 10px;
    height: 30px;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border-left: 1px solid var(--borda);
  }

  th:first-child,
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

  thead th {
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

  :global(:root.mica) thead th {
    background: color-mix(in srgb, var(--fundo) 92%, transparent);
  }

  thead th:nth-child(1) { width: 22%; }
  thead th:nth-child(2) { width: 72px; }
  thead th:nth-child(3) { width: 96px; }
  thead th:nth-child(4) { width: 112px; }
  thead th:nth-child(5) { width: 80px; }
  thead th:nth-child(6) { width: 104px; }

  thead button {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    width: 100%;
    height: 100%;
    padding: 6px 10px;
    border: 0;
    background: transparent;
    text-align: inherit;
    align-items: flex-start;
  }

  thead th.numero button {
    align-items: flex-end;
  }

  thead button:hover {
    background: var(--hover);
  }

  .total {
    font-size: 16px;
    font-variant-numeric: tabular-nums;
  }

  .titulo {
    color: var(--texto-2);
    font-size: 12px;
  }

  [aria-sort='ascending'] .titulo::after { content: ' ▲'; font-size: 9px; }
  [aria-sort='descending'] .titulo::after { content: ' ▼'; font-size: 9px; }

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

  td.nome {
    padding-left: 28px;
  }

  td.status,
  td.cmd {
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
