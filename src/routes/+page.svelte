<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { api, conectar, dentroDoApp } from '#lib/api.js';
  import Atividade from '#lib/Atividade.svelte';
  import AvisoAtualizacao from '#lib/AvisoAtualizacao.svelte';
  import AvisoRecursos from '#lib/AvisoRecursos.svelte';
  import Configuracoes from '#lib/Configuracoes.svelte';
  import Confirmar from '#lib/Confirmar.svelte';
  import Disco from '#lib/Disco.svelte';
  import Distros from '#lib/Distros.svelte';
  import Log from '#lib/Log.svelte';
  import Memoria from '#lib/Memoria.svelte';
  import { estado } from '#lib/estado.svelte.js';
  import Icone from '#lib/Icone.svelte';
  import Paleta from '#lib/Paleta.svelte';
  import Portas from '#lib/Portas.svelte';
  import Processador from '#lib/Processador.svelte';
  import Processos from '#lib/Processos.svelte';
  import type { Pagina } from '#lib/tipos.js';

  const PAGINAS: { id: Pagina; titulo: string }[] = [
    { id: 'processos', titulo: 'Processos' },
    { id: 'processador', titulo: 'Processador' },
    { id: 'memoria', titulo: 'Memória' },
    { id: 'disco', titulo: 'Disco' },
    { id: 'portas', titulo: 'Portas' },
    { id: 'atividade', titulo: 'Atividade' },
    { id: 'distros', titulo: 'Distros' },
    { id: 'configuracoes', titulo: 'Configurações' },
    { id: 'log', titulo: 'Log' },
  ];

  const VELOCIDADES = [
    { ms: 500, rotulo: 'Alta (0,5 s)' },
    { ms: 1000, rotulo: 'Normal (1 s)' },
    { ms: 4000, rotulo: 'Baixa (4 s)' },
  ];

  let expandido = $state(true);
  let pronto = $state(false);

  function atalho(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey)) return;
    const tecla = e.key.toLowerCase();
    if (tecla === 'k') {
      e.preventDefault();
      estado.paletaAberta = true;
    } else if (tecla === 'f') {
      // Na aba com filtro, Ctrl+F vai para ele em vez da busca de texto do WebView.
      const filtro = document.querySelector<HTMLInputElement>('main [data-filtro]');
      if (filtro) {
        e.preventDefault();
        filtro.focus();
        filtro.select();
      }
    }
  }

  async function mudarVelocidade(ms: number) {
    estado.intervalo = ms;
    try {
      await api.definirIntervalo(ms);
    } catch (e) {
      estado.avisar(String(e));
    }
  }

  onMount(async () => {
    if (dentroDoApp) document.documentElement.classList.add('mica');
    await conectar();
    pronto = true;
  });
</script>

<svelte:head>
  <title>Toca do Pinguim</title>
</svelte:head>

<svelte:window onkeydown={atalho} />

<div class="janela" class:expandido>
  <nav aria-label="Seções">
    <button
      class="item"
      aria-label={expandido ? 'Recolher menu' : 'Expandir menu'}
      aria-expanded={expandido}
      onclick={() => (expandido = !expandido)}
    >
      <Icone nome="menu" />
    </button>
    <button class="item busca" title={expandido ? undefined : 'Buscar (Ctrl+K)'} onclick={() => (estado.paletaAberta = true)}>
      <Icone nome="busca" />
      <span class="rotulo">Buscar</span>
      <kbd class="rotulo">Ctrl+K</kbd>
    </button>
    {#each PAGINAS as p (p.id)}
      <button
        class="item"
        aria-current={estado.pagina === p.id ? 'page' : undefined}
        title={expandido ? undefined : p.titulo}
        onclick={() => (estado.pagina = p.id)}
      >
        <Icone nome={p.id} />
        <span class="rotulo">{p.titulo}</span>
        {#if p.id === 'memoria' && estado.memoriaWindows?.pressao}
          <span class="alerta" role="img" aria-label="RAM do Windows acabando"></span>
        {/if}
      </button>
    {/each}
    <label class="velocidade" class:visualmente-oculto={!expandido}>
      <span>Velocidade de atualização</span>
      <select value={estado.intervalo} onchange={(e) => mudarVelocidade(Number(e.currentTarget.value))}>
        {#each VELOCIDADES as v (v.ms)}
          <option value={v.ms}>{v.rotulo}</option>
        {/each}
      </select>
    </label>
  </nav>

  <main>
    {#if estado.demonstracao}
      <p class="demonstracao" role="status">
        Modo demonstração: aberto fora do app, sem acesso ao WSL. Os dados são simulados.
      </p>
    {/if}
    {#if pronto}
      <AvisoAtualizacao />
      <AvisoRecursos />
      {#if estado.pagina === 'processos'}
        <Processos />
      {:else if estado.pagina === 'processador'}
        <Processador />
      {:else if estado.pagina === 'memoria'}
        <Memoria />
      {:else if estado.pagina === 'disco'}
        <Disco />
      {:else if estado.pagina === 'portas'}
        <Portas />
      {:else if estado.pagina === 'atividade'}
        <Atividade />
      {:else if estado.pagina === 'configuracoes'}
        <Configuracoes />
      {:else if estado.pagina === 'log'}
        <Log />
      {:else}
        <Distros />
      {/if}
    {/if}
  </main>

  <Confirmar />
  <Paleta />

  <div class="avisos" role="log" aria-live="polite">
    {#each estado.avisos as a (a.id)}
      <div class="aviso">
        <span>{a.texto}</span>
        <button class="botao sutil" aria-label="Dispensar aviso" onclick={() => estado.dispensar(a.id)}>
          <Icone nome="fechar" tamanho={12} />
        </button>
      </div>
    {/each}
  </div>
</div>

<style>
  .janela {
    display: grid;
    grid-template-columns: 48px 1fr;
    height: 100vh;
  }

  .janela.expandido {
    grid-template-columns: 220px 1fr;
  }

  @media (max-width: 760px) {
    .janela.expandido {
      grid-template-columns: 48px 1fr;
    }

    .janela.expandido .rotulo {
      display: none;
    }
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 4px;
    min-width: 0;
  }

  .item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    height: 36px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--raio);
    background: transparent;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
  }

  .item:hover {
    background: var(--hover);
  }

  .item.busca {
    margin-bottom: 6px;
    border: 1px solid var(--borda);
  }

  .item.busca kbd {
    margin-left: auto;
    color: var(--texto-2);
    font: inherit;
    font-size: 11px;
  }

  .item[aria-current='page'] {
    background: var(--hover);
  }

  .item[aria-current='page']::before {
    content: '';
    position: absolute;
    left: 0;
    top: 10px;
    bottom: 10px;
    width: 3px;
    border-radius: 2px;
    background: var(--destaque);
  }

  .alerta {
    flex: none;
    width: 8px;
    height: 8px;
    margin-left: auto;
    border-radius: 50%;
    background: rgb(var(--calor));
  }

  .janela:not(.expandido) .alerta {
    position: absolute;
    top: 6px;
    left: 26px;
  }

  .janela:not(.expandido) .rotulo {
    display: none;
  }

  .velocidade {
    display: grid;
    gap: 4px;
    margin-top: auto;
    padding: 10px 8px;
    color: var(--texto-2);
    font-size: 12px;
  }

  .velocidade select {
    height: 30px;
    padding: 0 6px;
    border: 1px solid var(--borda);
    border-radius: var(--raio);
    background: var(--camada-forte);
  }

  main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border-top: 1px solid var(--borda);
    border-left: 1px solid var(--borda);
    border-top-left-radius: 8px;
    background: var(--camada);
  }

  .demonstracao {
    margin: 12px 20px 0;
    padding: 8px 12px;
    border-radius: var(--raio);
    background: rgba(var(--calor), 0.18);
    font-size: 13px;
  }

  .avisos {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 20;
    display: grid;
    gap: 8px;
    width: min(420px, calc(100vw - 32px));
  }

  .aviso {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 8px 14px;
    border: 1px solid var(--borda-forte);
    border-left: 3px solid var(--perigo);
    border-radius: var(--raio);
    background: var(--camada-forte);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.18);
    font-size: 13px;
  }

  .aviso span {
    flex: 1;
  }
</style>
