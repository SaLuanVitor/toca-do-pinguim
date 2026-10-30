<script lang="ts">
  // Os tetos da VM do WSL num lugar só: memória e processadores, que vão para o .wslconfig, e o
  // cache que o app devolve sozinho ao Windows.
  import { onMount } from 'svelte';
  import { api } from './api';
  import { estado } from './estado.svelte';
  import { bytes } from './formato';
  import type { ResumoMemoria } from './tipos';

  let { resumo, onajustar }: { resumo: ResumoMemoria | null; onajustar: () => void } = $props();

  const GIB = 1024 ** 3;
  const PORCENTAGENS = [5, 10, 15, 20, 25, 30, 40, 50];

  let maquina = $state(0);
  // '' é o padrão do WSL: todos os processadores lógicos.
  let processadores = $state('');
  let gravado = $state('');
  let salvandoCpu = $state(false);

  let cacheGb = $state('');
  let salvandoCache = $state(false);

  const ramGb = $derived(Math.floor((estado.memoriaWindows?.total_bytes ?? 0) / GIB));
  const cacheValido = $derived.by(() => {
    if (cacheGb.trim() === '') return true;
    const n = Number(cacheGb.replace(',', '.'));
    return Number.isFinite(n) && n >= 0.5 && (ramGb === 0 || n <= ramGb);
  });

  onMount(async () => {
    try {
      const [limites, config] = await Promise.all([api.limitesDaMaquina(), api.lerWslconfig()]);
      maquina = limites.processadores;
      const atual = config.valores.find(([s, k]) => s.toLowerCase() === 'wsl2' && k.toLowerCase() === 'processors');
      processadores = gravado = atual ? atual[2].trim() : '';
    } catch (e) {
      estado.avisar(String(e));
    }
  });

  $effect(() => {
    const maximo = estado.gatilho.cache_maximo_bytes;
    cacheGb = maximo === null ? '' : String(Math.round((maximo / GIB) * 10) / 10).replace('.', ',');
  });

  async function salvarProcessadores(reiniciar: boolean) {
    salvandoCpu = true;
    try {
      await api.gravarWslconfig([{ secao: 'wsl2', chave: 'processors', valor: processadores || null }], reiniciar);
      gravado = processadores;
      if (reiniciar) estado.receberDistros(await api.listarDistros());
    } catch (e) {
      estado.avisar(String(e));
    } finally {
      salvandoCpu = false;
    }
  }

  function salvarProcessadoresEReiniciar() {
    const ligadas = estado.distros.filter((d) => d.rodando).map((d) => d.nome);
    estado.pedido = {
      titulo: 'Reiniciar o WSL agora?',
      texto: ligadas.length
        ? `Para o novo limite de processadores valer, a VM do WSL precisa reiniciar. Serão terminadas: ${ligadas.join(', ')}.`
        : 'Nenhuma distro está ligada; o novo limite vale na próxima vez que uma ligar.',
      acao: 'Salvar e reiniciar',
      executar: () => salvarProcessadores(true),
    };
  }

  /** Devolve false quando o app recusou; quem chamou volta o controle ao que vale. */
  async function salvarCache(automatico: boolean, minimo: number, gb: string): Promise<boolean> {
    const maximo = gb.trim() === '' ? null : Number(gb.replace(',', '.'));
    salvandoCache = true;
    try {
      await api.definirDevolucao(automatico, minimo, maximo);
      estado.devolverCacheAutomatico = automatico;
      estado.gatilho = { minimo_livre_pct: minimo, cache_maximo_bytes: maximo === null ? null : maximo * GIB };
      return true;
    } catch (e) {
      estado.avisar(String(e));
      estado.gatilho = { ...estado.gatilho };
      return false;
    } finally {
      salvandoCache = false;
    }
  }
</script>

<section class="cartao largo" aria-labelledby="titulo-limites">
  <div class="cabeca">
    <h2 id="titulo-limites">Limites</h2>
    <span>Quanto a VM do WSL pode tirar do Windows</span>
  </div>

  <div class="grade">
    <div class="bloco" role="group" aria-labelledby="limite-mem">
      <h3 id="limite-mem">Memória</h3>
      {#if resumo}
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
        <p class="explica">A VM não passa desse teto: acima dele o Linux usa o swap e, sem swap livre, encerra processos.</p>
        <div class="acoes"><button class="botao" onclick={onajustar}>Ajustar limite</button></div>
      {:else}
        <p class="explica">Carregando o histórico de memória.</p>
      {/if}
    </div>

    <div class="bloco" role="group" aria-labelledby="limite-cpu">
      <h3 id="limite-cpu">Processadores</h3>
      <label class="campo">
        <span>A VM pode usar</span>
        <select bind:value={processadores} disabled={!maquina}>
          <option value="">Todos ({maquina || '?'}, padrão do WSL)</option>
          {#each Array.from({ length: maquina }, (_, i) => String(i + 1)) as n (n)}
            <option value={n}>{n} de {maquina}</option>
          {/each}
        </select>
      </label>
      <p class="explica">
        Com menos processadores, um build ou um container que dispara não leva a máquina toda: a VM fica com até
        esse número de núcleos e o resto do Windows continua respondendo.{estado.vm ? ` Hoje a VM vê ${estado.vm.cpus}.` : ''}
        Vale quando o WSL reiniciar.
      </p>
      <div class="acoes">
        <button
          class="botao primario"
          disabled={processadores === gravado || salvandoCpu}
          onclick={() => salvarProcessadores(false)}
        >
          Salvar para o próximo início
        </button>
        <button class="botao" disabled={processadores === gravado || salvandoCpu} onclick={salvarProcessadoresEReiniciar}>
          Salvar e reiniciar o WSL
        </button>
      </div>
    </div>

    <div class="bloco" role="group" aria-labelledby="limite-cache">
      <h3 id="limite-cache">Cache</h3>
      <label class="opcao">
        <input
          type="checkbox"
          checked={estado.devolverCacheAutomatico}
          disabled={salvandoCache}
          onchange={async (e) => {
            const caixa = e.currentTarget;
            if (!(await salvarCache(caixa.checked, estado.gatilho.minimo_livre_pct, cacheGb))) caixa.checked = !caixa.checked;
          }}
        />
        Devolver o cache da VM sozinho
      </label>
      <label class="campo">
        <span>Quando a memória livre do Windows ficar abaixo de</span>
        <select
          value={estado.gatilho.minimo_livre_pct}
          disabled={!estado.devolverCacheAutomatico || salvandoCache}
          onchange={async (e) => {
            const lista = e.currentTarget;
            if (!(await salvarCache(true, Number(lista.value), cacheGb))) lista.value = String(estado.gatilho.minimo_livre_pct);
          }}
        >
          {#each PORCENTAGENS as p (p)}
            <option value={p}>{p}%</option>
          {/each}
        </select>
      </label>
      <label class="campo">
        <span>Ou quando o cache da VM passar de (GB, vazio para não olhar)</span>
        <input
          type="text"
          inputmode="decimal"
          value={cacheGb}
          placeholder="não olhar"
          aria-invalid={!cacheValido}
          disabled={!estado.devolverCacheAutomatico || salvandoCache}
          oninput={(e) => (cacheGb = e.currentTarget.value = e.currentTarget.value.replace(/[^\d,.]/g, ''))}
          onchange={() => cacheValido && salvarCache(true, estado.gatilho.minimo_livre_pct, cacheGb)}
        />
      </label>
      {#if !cacheValido}
        <p class="erro" role="alert">Use um número de 0,5 a {ramGb} GB, ou deixe vazio.</p>
      {/if}
      <p class="explica">
        Basta um dos dois. No máximo uma vez a cada 10 minutos, com o resultado na Atividade. Nada se perde: a VM só
        relê do disco o que precisar. Devolver não impede o cache de crescer de novo; para a VM ocupar menos de vez,
        baixe o limite de memória.
      </p>
    </div>
  </div>
</section>

<style>
  .cartao {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    border: 1px solid var(--borda);
    border-radius: 8px;
    background: var(--camada);
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

  h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }

  .grade {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 20px;
  }

  .bloco {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  dl {
    display: flex;
    gap: 24px;
    flex-wrap: wrap;
    margin: 0;
  }

  dt {
    color: var(--texto-2);
    font-size: 12px;
  }

  dd {
    margin: 2px 0 0;
    font-size: 16px;
    font-variant-numeric: tabular-nums;
  }

  .campo {
    display: grid;
    gap: 6px;
  }

  .campo span {
    color: var(--texto-2);
    font-size: 12px;
  }

  .campo select,
  .campo input {
    height: 32px;
    max-width: 240px;
    padding: 0 10px;
    border: 1px solid var(--borda);
    border-bottom-color: var(--borda-forte);
    border-radius: var(--raio);
    background: var(--camada-forte);
    color: var(--texto);
  }

  .campo input[aria-invalid='true'] {
    border-bottom: 2px solid var(--perigo);
  }

  .opcao {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .explica {
    margin: 0;
    color: var(--texto-2);
    font-size: 12px;
  }

  .erro {
    margin: 0;
    color: var(--perigo);
    font-size: 12px;
  }

  .acoes {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: auto;
  }
</style>
