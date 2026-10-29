<script lang="ts">
  // Escolhe o arquivo a editar: o .wslconfig, que vale para todas as distros, ou o
  // /etc/wsl.conf de uma distro. Ler o de uma distro desligada a liga, então isso só
  // acontece a pedido.
  import { api } from './api';
  import { CATALOGO, GRUPOS } from './catalogoWslconfig';
  import { CATALOGO_WSLCONF, GRUPOS_WSLCONF } from './catalogoWslconf';
  import EditorChaves from './EditorChaves.svelte';
  import { estado } from './estado.svelte';
  import type { MudancaWslconfig } from './tipos';

  const TODAS = '';

  let escopo = $state(TODAS);
  let filtro = $state('');
  let liberadas = $state<string[]>([]);

  const distro = $derived(estado.distros.find((d) => d.nome === escopo));
  const legivel = $derived(escopo === TODAS || !!distro?.rodando || liberadas.includes(escopo));

  // Uma vez lido, o editor da distro fica aberto mesmo que ela desligue (por ociosidade ou
  // por "Salvar e reiniciar"): remontá-lo releria o arquivo e ligaria a distro de novo.
  $effect(() => {
    if (escopo !== TODAS && legivel && !liberadas.includes(escopo)) liberadas = [...liberadas, escopo];
  });

  async function atualizarDistros() {
    estado.receberDistros(await api.listarDistros());
  }

  async function gravarGlobal(mudancas: MudancaWslconfig[], reiniciar: boolean) {
    await api.gravarWslconfig(mudancas, reiniciar);
    if (reiniciar) await atualizarDistros();
  }

  async function gravarDistro(mudancas: MudancaWslconfig[], reiniciar: boolean) {
    await api.gravarWslconf(escopo, mudancas, reiniciar);
    if (reiniciar) await atualizarDistros();
  }

  const reiniciarGlobal = {
    botao: 'Salvar e reiniciar o WSL',
    titulo: 'Salvar e reiniciar o WSL?',
    texto: () => {
      const ligadas = estado.distros.filter((d) => d.rodando).map((d) => d.nome);
      return `O .wslconfig é gravado e a VM do WSL é desligada, terminando todas as distros (${ligadas.join(', ') || 'nenhuma ligada agora'}).`;
    },
    feito: 'o WSL foi desligado, e a configuração vale na próxima distro que ligar.',
  };

  const reiniciarDistro = $derived({
    botao: `Salvar e reiniciar ${escopo}`,
    titulo: `Salvar e reiniciar ${escopo}?`,
    texto: () =>
      `O wsl.conf é gravado e ${escopo} é terminada, encerrando todos os processos dela. As outras distros continuam como estão.`,
    feito: `${escopo} foi terminada e liga com a configuração nova na próxima vez que for usada.`,
  });
</script>

<header class="barra">
  <h1>Configurações</h1>
  <label class="escopo">
    <span>Aplicar a</span>
    <select bind:value={escopo}>
      <option value={TODAS}>Todas as distros (.wslconfig)</option>
      {#each estado.distros as d (d.nome)}
        <option value={d.nome}>{d.nome} (wsl.conf){d.rodando ? '' : ', desligada'}</option>
      {/each}
    </select>
  </label>
  <input
    class="busca"
    type="search"
    placeholder="Filtrar configurações"
    aria-label="Filtrar configurações"
    data-filtro
    bind:value={filtro}
    onkeydown={(e) => e.key === 'Escape' && (filtro = '')}
  />
</header>

{#if escopo === TODAS}
  {#key escopo}
    <EditorChaves
      catalogo={CATALOGO}
      grupos={GRUPOS}
      {filtro}
      quandoVale="Vale para todas as distros, e só depois que o WSL reinicia."
      ler={() => api.lerWslconfig()}
      gravar={gravarGlobal}
      reiniciar={reiniciarGlobal}
    />
  {/key}
{:else if legivel}
  {#key escopo}
    <EditorChaves
      catalogo={CATALOGO_WSLCONF}
      grupos={GRUPOS_WSLCONF}
      {filtro}
      quandoVale="Vale só para {escopo}, e só depois que ela reinicia."
      ler={() => api.lerWslconf(escopo)}
      gravar={gravarDistro}
      reiniciar={reiniciarDistro}
    />
  {/key}
{:else}
  <div class="vazio">
    <strong>{escopo} está desligada</strong>
    <span>
      O wsl.conf fica dentro da distro, e lê-lo liga {escopo}. Sem uso, ela desliga sozinha de novo
      depois de alguns segundos.
    </span>
    <button class="botao primario" onclick={() => (liberadas = [...liberadas, escopo])}>Ligar e ler</button>
  </div>
{/if}

<style>
  .escopo {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--texto-2);
    font-size: 13px;
  }

  .escopo select,
  .busca {
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--borda);
    border-radius: var(--raio);
    background: var(--camada-forte);
    color: var(--texto);
  }

  .busca {
    width: min(240px, 100%);
  }
</style>
