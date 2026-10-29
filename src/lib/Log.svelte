<script lang="ts">
  // Mostra o fim do log técnico em arquivo. Por padrão só avisos e erros, que é onde caem
  // as falhas do WSL; "Tudo" inclui o que o app fez e quando.
  import { onMount } from 'svelte';
  import { api } from './api';
  import { estado } from './estado.svelte';
  import Icone from './Icone.svelte';

  const LINHAS = 2000;
  const A_CADA_MS = 5000;

  type Nivel = 'ERROR' | 'WARN' | 'INFO' | 'DEBUG' | 'TRACE';
  interface Linha {
    quando: string;
    nivel: Nivel | null;
    texto: string;
    bruta: string;
  }

  const ROTULOS: Record<Nivel, string> = {
    ERROR: 'Erro',
    WARN: 'Aviso',
    INFO: 'Info',
    DEBUG: 'Depuração',
    TRACE: 'Rastro',
  };

  // Formato do tauri-plugin-log: [data][hora][NÍVEL][módulo] mensagem
  const PADRAO = /^\[([^\]]+)\]\[([^\]]+)\]\[(ERROR|WARN|INFO|DEBUG|TRACE)\]\[[^\]]*\] ?(.*)$/;

  function interpretar(bruta: string): Linha {
    const m = PADRAO.exec(bruta);
    if (!m) return { quando: '', nivel: null, texto: bruta, bruta };
    return { quando: `${m[1]} ${m[2]}`, nivel: m[3] as Nivel, texto: m[4], bruta };
  }

  let linhas = $state<Linha[]>([]);
  let soProblemas = $state(true);
  let copiado = $state(false);

  const visiveis = $derived(
    linhas.filter((l) => !soProblemas || l.nivel === 'ERROR' || l.nivel === 'WARN').toReversed(),
  );

  async function carregar() {
    try {
      linhas = (await api.lerLog(LINHAS)).map(interpretar);
    } catch (e) {
      estado.avisar(`Não foi possível ler o log: ${e}`);
    }
  }

  async function copiar() {
    try {
      await navigator.clipboard.writeText(visiveis.toReversed().map((l) => l.bruta).join('\n'));
      copiado = true;
      setTimeout(() => (copiado = false), 2000);
    } catch (e) {
      estado.avisar(`Não foi possível copiar: ${e}`);
    }
  }

  async function abrirPasta() {
    try {
      await api.abrirPastaDeLogs();
    } catch (e) {
      estado.avisar(String(e));
    }
  }

  onMount(() => {
    carregar();
    const id = setInterval(carregar, A_CADA_MS);
    return () => clearInterval(id);
  });
</script>

<header class="barra">
  <h1>Log</h1>
  <div class="filtros" role="group" aria-label="Filtrar o log">
    <button class="botao" aria-pressed={soProblemas} onclick={() => (soProblemas = true)}>Problemas</button>
    <button class="botao" aria-pressed={!soProblemas} onclick={() => (soProblemas = false)}>Tudo</button>
  </div>
  <div class="acoes">
    <button class="botao" onclick={copiar} disabled={visiveis.length === 0}>{copiado ? 'Copiado' : 'Copiar'}</button>
    <button class="botao" onclick={abrirPasta}><Icone nome="pasta" /> Abrir pasta</button>
  </div>
</header>

<div class="rolagem">
  {#if visiveis.length === 0}
    <div class="vazio">
      <strong>{soProblemas ? 'Nenhum problema registrado' : 'Log vazio'}</strong>
      <span>
        Falhas e lentidão do WSL, agentes que não subiram e erros do app aparecem aqui. O arquivo
        fica em %LOCALAPPDATA%\com.luan.tocadopinguim\logs.
      </span>
    </div>
  {:else}
    <ol>
      {#each visiveis as l, i (`${i}|${l.bruta}`)}
        <li class:erro={l.nivel === 'ERROR'} class:aviso={l.nivel === 'WARN'}>
          <time>{l.quando}</time>
          <span class="nivel">{l.nivel ? ROTULOS[l.nivel] : ''}</span>
          <code>{l.texto}</code>
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  .filtros {
    display: flex;
    gap: 4px;
  }

  .filtros .botao[aria-pressed='true'] {
    background: var(--selecionado);
    border-color: var(--destaque);
    color: var(--destaque);
  }

  .rolagem {
    flex: 1;
    overflow: auto;
    padding: 0 20px 20px;
  }

  ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: 150px 72px 1fr;
    gap: 12px;
    align-items: baseline;
    padding: 6px 10px;
    border-bottom: 1px solid var(--borda);
    font-size: 12px;
  }

  li.erro {
    border-left: 3px solid var(--perigo);
  }

  li.aviso {
    border-left: 3px solid rgb(var(--calor));
  }

  time,
  .nivel {
    color: var(--texto-2);
    font-variant-numeric: tabular-nums;
  }

  li.erro .nivel {
    color: var(--perigo);
  }

  code {
    font-family: 'Cascadia Mono', Consolas, monospace;
    white-space: pre-wrap;
    word-break: break-word;
  }

  @media (max-width: 640px) {
    li {
      grid-template-columns: 1fr;
      gap: 2px;
    }
  }
</style>
