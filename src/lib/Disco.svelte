<script lang="ts">
  // Onde está o espaço do WSL e o que dá para liberar. Toda limpeza passa pela confirmação e
  // vai para a Atividade com o quanto liberou.
  import { onMount } from 'svelte';
  import { api } from './api';
  import { estado } from './estado.svelte';
  import { bytes } from './formato';
  import type {
    AnalisePastas,
    Armazenamento,
    DiscoVirtual,
    LimpezaDocker,
    LimpezaProgramada,
    Pasta,
    ProjetoDocker,
    VolumeProjeto,
  } from './tipos';

  let dados = $state<Armazenamento | null>(null);
  let lendo = $state(false);
  let limpando = $state<string | null>(null);
  let resultado = $state('');

  let distro = $state('');
  let analise = $state<AnalisePastas | null>(null);
  let analisando = $state(false);
  let marcadas = $state<Record<string, boolean>>({});

  let compactando = $state<string | null>(null);

  let volumesAbertos = $state(false);
  let volumesMarcados = $state<Record<string, boolean>>({});

  const data = new Intl.DateTimeFormat('pt-BR', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' });

  const NOMES: Record<string, string> = {
    Images: 'Imagens',
    Containers: 'Containers',
    'Local Volumes': 'Volumes',
    'Build Cache': 'Cache de build',
  };

  async function ler() {
    lendo = true;
    try {
      dados = await api.lerArmazenamento();
      if (!dados.distros.includes(distro)) distro = dados.distros_ligadas[0] ?? dados.distros[0] ?? '';
      volumesMarcados = {};
    } catch (e) {
      estado.avisar(String(e));
    } finally {
      lendo = false;
    }
  }

  function categoria(tipo: string) {
    return dados?.docker?.categorias.find((c) => c.tipo === tipo);
  }

  function limparDocker(qual: LimpezaDocker, titulo: string, texto: string, volumes: string[] = []) {
    estado.pedido = {
      titulo,
      texto,
      acao: 'Apagar',
      executar: async () => {
        limpando = qual;
        resultado = '';
        try {
          const liberado = await api.limparDocker(qual, volumes);
          resultado = `${titulo.replace(/\?$/, '')}: ${bytes(liberado)} liberados dentro do disco do Docker.`;
          await ler();
        } catch (e) {
          estado.avisar(String(e));
        } finally {
          limpando = null;
        }
      },
    };
  }

  function compactar(d: DiscoVirtual) {
    if (estado.docker.ligado) {
      estado.avisar('Feche o Docker Desktop antes de compactar: ele religa o WSL logo depois e prende o disco.');
      return;
    }
    const ligadas = estado.distros.filter((x) => x.rodando).map((x) => x.nome);
    const vazio = d.usado_bytes !== null ? Math.max(0, d.tamanho_bytes - d.usado_bytes) : null;
    estado.pedido = {
      titulo: `Compactar o disco de ${d.nome}?`,
      texto: [
        ligadas.length
          ? `O WSL inteiro será desligado, e ${ligadas.join(', ')} ${ligadas.length === 1 ? 'para' : 'param'}.`
          : 'O WSL já está desligado.',
        'O Windows vai pedir permissão de administrador para o diskpart, e a compactação pode levar vários minutos; não ligue nenhuma distro até ela acabar.',
        vazio !== null ? `Pode devolver até ${bytes(vazio)} ao Windows.` : 'Quanto volta só se sabe no fim.',
        'Nada dentro do disco é apagado.',
      ].join(' '),
      acao: 'Desligar e compactar',
      executar: async () => {
        compactando = d.caminho;
        resultado = `Compactando o disco de ${d.nome}: aceite o pedido de administrador do Windows. Pode levar vários minutos.`;
        try {
          const r = await api.compactarDisco(d.caminho);
          const devolvido = Math.max(0, r.antes_bytes - r.depois_bytes);
          resultado = devolvido
            ? `Disco de ${d.nome} compactado: de ${bytes(r.antes_bytes)} para ${bytes(r.depois_bytes)}, ${bytes(devolvido)} devolvidos ao Windows. O WSL ficou desligado.`
            : `O disco de ${d.nome} não tinha espaço a devolver. O WSL ficou desligado.`;
          estado.receberDistros(await api.listarDistros());
          await ler();
        } catch (e) {
          resultado = '';
          estado.avisar(String(e));
        } finally {
          compactando = null;
        }
      },
    };
  }

  const volumesEscolhidos = $derived(dados?.docker?.volumes_sem_uso.filter((v) => volumesMarcados[v.nome]) ?? []);

  const ligada = (d: string) => !!dados?.distros_ligadas.includes(d);

  // O Docker só aparece na máquina que o tem: disco de dados dele, distro dele ou leitura dele.
  const temDocker = $derived(
    !!dados && (!!dados.docker || !!dados.erro_docker || dados.discos.some((d) => d.nome.startsWith('Docker') || d.nome.startsWith('docker-desktop'))),
  );

  async function rodarAnalise(alvo: string, ligar: boolean) {
    analisando = true;
    analise = null;
    marcadas = {};
    try {
      analise = await api.analisarPastas(alvo, ligar);
      if (ligar) await ler();
    } catch (e) {
      estado.avisar(String(e));
    } finally {
      analisando = false;
    }
  }

  function analisar() {
    if (!distro) return;
    if (ligada(distro)) {
      rodarAnalise(distro, false);
      return;
    }
    const alvo = distro;
    estado.pedido = {
      titulo: `Ligar ${alvo} para analisar?`,
      texto: `${alvo} está desligada, e a análise precisa dela ligada. O app a liga só para isso; quando ela ficar sem uso, o WSL a desliga sozinho, como faz com qualquer distro.`,
      acao: 'Ligar e analisar',
      executar: () => rodarAnalise(alvo, true),
    };
  }

  function apagarParadosDoProjeto(p: ProjetoDocker) {
    estado.pedido = {
      titulo: `Apagar os containers parados de ${p.nome}?`,
      texto: `Apaga ${p.parados} container(s) parado(s) do projeto ${p.nome}. Volumes e imagens ficam; para usar o projeto de novo, docker compose up recria os containers.`,
      acao: 'Apagar',
      executar: async () => {
        limpando = `parados:${p.nome}`;
        resultado = '';
        try {
          const liberado = await api.limparParadosDoProjeto(p.nome);
          resultado = `Containers parados de ${p.nome} apagados: ${bytes(liberado)} liberados dentro do disco do Docker.`;
          await ler();
        } catch (e) {
          estado.avisar(String(e));
        } finally {
          limpando = null;
        }
      },
    };
  }

  function apagarVolumesDoProjeto(projeto: string, volumes: VolumeProjeto[]) {
    limparDocker(
      'volumes',
      `Apagar ${volumes.length} volume(s) sem uso de ${projeto}?`,
      `Apaga ${volumes.map((v) => v.nome).join(', ')}, somando ${bytes(volumes.reduce((s, v) => s + v.bytes, 0))}. Nenhum container os usa, mas os dados deles se perdem de vez.`,
      volumes.map((v) => v.nome),
    );
  }

  const escolhidas = $derived(analise ? [...analise.tmp, ...analise.caches].filter((p) => marcadas[p.caminho]) : []);
  const escolhidasBytes = $derived(escolhidas.reduce((s, p) => s + p.bytes, 0));

  function apagarPastas() {
    if (!analise || escolhidas.length === 0) return;
    const alvo = analise.distro;
    const lista = escolhidas.map((p) => p.caminho);
    estado.pedido = {
      titulo: `Apagar ${lista.length === 1 ? '1 item' : `${lista.length} itens`} de ${alvo}?`,
      texto: `Somam ${bytes(escolhidasBytes)}: ${lista.join(', ')}. Entrada do /tmp sai inteira; pasta de cache é esvaziada e o programa gera de novo o que precisar. Não há lixeira: o que estiver no /tmp se perde.`,
      acao: 'Apagar',
      executar: async () => {
        limpando = 'pastas';
        try {
          const liberado = await api.apagarPastas(alvo, lista);
          resultado = `${bytes(liberado)} liberados dentro do disco de ${alvo}.`;
          if (analise) {
            analise = {
              ...analise,
              tmp: analise.tmp.filter((p) => !lista.includes(p.caminho)),
              caches: analise.caches.map((p) => (lista.includes(p.caminho) ? { ...p, bytes: 0 } : p)),
            };
          }
          marcadas = {};
        } catch (e) {
          estado.avisar(String(e));
        } finally {
          limpando = null;
        }
      },
    };
  }

  async function mudarLimpeza(parcial: Partial<LimpezaProgramada>) {
    if (!dados) return;
    const nova = { ...dados.limpeza, ...parcial };
    try {
      await api.definirLimpeza(nova);
      dados = { ...dados, limpeza: nova };
    } catch (e) {
      estado.avisar(String(e));
    }
  }

  function idade(p: Pasta): string {
    return p.modificado_ms ? data.format(p.modificado_ms) : '';
  }

  onMount(ler);
</script>

<div class="rolagem">
  <div class="topo">
    <button class="botao" onclick={ler} disabled={lendo}>{lendo ? 'Lendo' : 'Atualizar'}</button>
  </div>
  {#if resultado}<p class="resultado" role="status">{resultado}</p>{/if}

  <section class="cartao" aria-labelledby="titulo-discos">
    <div class="cabeca">
      <h2 id="titulo-discos">Discos virtuais</h2>
      <span>Cada distro guarda os arquivos num disco virtual (.vhdx) no Windows</span>
    </div>
    {#if !dados}
      <p class="nota">{lendo ? 'Lendo os discos e o Docker. Pode levar alguns segundos.' : 'Sem dados.'}</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th scope="col">Disco</th>
            <th scope="col" class="numero">No Windows</th>
            <th scope="col" class="numero">Usado por dentro</th>
            <th scope="col" class="numero">Espaço vazio no arquivo</th>
            <th scope="col">Arquivo</th>
            <th scope="col"><span class="visualmente-oculto">Ação</span></th>
          </tr>
        </thead>
        <tbody>
          {#each dados.discos as d (d.caminho)}
            <tr>
              <th scope="row">{d.nome}{#if d.esparso}<small>esparso</small>{/if}</th>
              <td class="numero">{bytes(d.tamanho_bytes)}</td>
              <td class="numero">{d.usado_bytes !== null ? bytes(d.usado_bytes) : ''}</td>
              <td class="numero">{d.usado_bytes !== null ? bytes(Math.max(0, d.tamanho_bytes - d.usado_bytes)) : ''}</td>
              <td class="caminho" title={d.caminho}>{d.caminho}</td>
              <td class="acao">
                {#if d.compactavel}
                  <button class="botao" disabled={!!compactando} onclick={() => compactar(d)}
                    >{compactando === d.caminho ? 'Compactando' : 'Compactar'}</button
                  >
                {/if}
              </td>
            </tr>
            {#if d.nota}
              <tr class="nota-linha"><td colspan="6">{d.nota}</td></tr>
            {/if}
          {/each}
        </tbody>
      </table>
      <p class="nota">
        O uso por dentro só aparece com a distro ligada. Apagar algo dentro da distro libera espaço nela, mas o arquivo no Windows
        não diminui sozinho: o espaço vazio dentro dele só volta ao Windows compactando o disco. Compactar desliga o WSL e
        precisa do Docker Desktop fechado.
      </p>
    {/if}
  </section>

  <section class="cartao" aria-labelledby="titulo-pastas">
    <div class="cabeca">
      <h2 id="titulo-pastas">Dentro de uma distro</h2>
      <span>As pastas que mais ocupam o disco virtual</span>
    </div>
    {#if dados && dados.distros.length === 0}
      <p class="nota">Nenhuma distro instalada.</p>
    {:else}
      <div class="analisar">
        <label>
          Distro
          <select bind:value={distro}>
            {#each dados?.distros ?? [] as d (d)}<option value={d}>{d}{ligada(d) ? '' : ' (desligada)'}</option>{/each}
          </select>
        </label>
        <button class="botao" onclick={analisar} disabled={analisando || !distro}
          >{analisando ? 'Analisando' : ligada(distro) ? 'Analisar' : 'Ligar e analisar'}</button
        >
        {#if analisando}<span class="nota">Percorre o disco inteiro da distro; pode levar alguns minutos.</span>{/if}
      </div>
      {#if analise}
        <h3>Maiores pastas da raiz</h3>
        <ul class="raiz">
          {#each analise.raiz as p (p.caminho)}<li><code>{p.caminho}</code> <span class="numero">{bytes(p.bytes)}</span></li>{/each}
        </ul>

        <h3>/tmp</h3>
        <p class="nota">
          O /tmp do WSL não se esvazia nem ao reiniciar. Pasta montada por um container rodando aparece bloqueada: apagá-la
          quebraria o container.
        </p>
        <table>
          <thead>
            <tr>
              <th scope="col"><span class="visualmente-oculto">Marcar</span></th>
              <th scope="col">Pasta</th>
              <th scope="col" class="numero">Tamanho</th>
              <th scope="col">Mudou em</th>
              <th scope="col">Situação</th>
            </tr>
          </thead>
          <tbody>
            {#each analise.tmp as p (p.caminho)}
              <tr class:bloqueada={!p.apagavel}>
                <td><input type="checkbox" aria-label="Marcar {p.caminho}" disabled={!p.apagavel} bind:checked={marcadas[p.caminho]} /></td>
                <td class="caminho" title={p.caminho}><code>{p.caminho}</code></td>
                <td class="numero">{bytes(p.bytes)}</td>
                <td>{idade(p)}</td>
                <td>{p.em_uso_por ? `em uso por ${p.em_uso_por}` : ''}</td>
              </tr>
            {/each}
          </tbody>
        </table>

        <h3>Pastas de cache em disco</h3>
        <ul class="raiz">
          {#each analise.caches as p (p.caminho)}
            <li>
              <label><input type="checkbox" disabled={!p.apagavel || p.bytes === 0} bind:checked={marcadas[p.caminho]} /> <code>{p.caminho}</code></label>
              <span class="numero">{bytes(p.bytes)}</span>
            </li>
          {/each}
        </ul>
        <div class="analisar">
          <button class="botao perigo" disabled={!!limpando || escolhidas.length === 0} onclick={apagarPastas}>
            {limpando === 'pastas' ? 'Apagando' : `Apagar os marcados (${bytes(escolhidasBytes)})`}
          </button>
        </div>
      {/if}
    {/if}
  </section>

  {#if temDocker}
  <section class="cartao" aria-labelledby="titulo-docker-armazenamento">
    <div class="cabeca">
      <h2 id="titulo-docker-armazenamento">Espaço do Docker</h2>
      <span>Recuperável segundo o próprio Docker</span>
    </div>
    {#if dados && !dados.docker && !dados.erro_docker}
      <p class="nota">O Docker Desktop não está ligado.</p>
    {:else if dados?.erro_docker}
      <p class="nota">Não foi possível ler o Docker: {dados.erro_docker}</p>
    {:else if dados?.docker}
      {@const imagens = categoria('Images')}
      {@const containers = categoria('Containers')}
      {@const volumes = categoria('Local Volumes')}
      {@const build = categoria('Build Cache')}
      <table>
        <thead>
          <tr>
            <th scope="col">O quê</th>
            <th scope="col" class="numero">Total</th>
            <th scope="col" class="numero">Recuperável</th>
            <th scope="col"><span class="visualmente-oculto">Ação</span></th>
          </tr>
        </thead>
        <tbody>
          {#if imagens}
            <tr>
              <th scope="row">{NOMES[imagens.tipo]} <small>{imagens.quantidade}, {imagens.ativos} em uso</small></th>
              <td class="numero">{bytes(imagens.total_bytes)}</td>
              <td class="numero">{bytes(imagens.recuperavel_bytes)}</td>
              <td class="acao">
                <button
                  class="botao"
                  disabled={!!limpando || imagens.recuperavel_bytes === 0}
                  onclick={() =>
                    limparDocker(
                      'imagens_sem_uso',
                      'Apagar as imagens sem uso?',
                      `Apaga as imagens que nenhum container usa, cerca de ${bytes(imagens.recuperavel_bytes)}. Um projeto que precisar de uma delas baixa ou monta de novo, o que leva tempo e rede.`,
                    )}>{limpando === 'imagens_sem_uso' ? 'Apagando' : 'Apagar sem uso'}</button
                >
              </td>
            </tr>
          {/if}
          {#if containers}
            <tr>
              <th scope="row">{NOMES[containers.tipo]} <small>{containers.quantidade}, {containers.ativos} rodando</small></th>
              <td class="numero">{bytes(containers.total_bytes)}</td>
              <td class="numero">{bytes(containers.recuperavel_bytes)}</td>
              <td class="acao">
                <button
                  class="botao"
                  disabled={!!limpando || dados.docker.sobras === 0}
                  title="Containers parados sem projeto do Compose: {dados.docker.sobras}, {bytes(dados.docker.sobras_bytes)}"
                  onclick={() =>
                    limparDocker(
                      'sobras',
                      'Apagar os containers parados de builds?',
                      `Apaga ${dados?.docker?.sobras} containers parados que não pertencem a nenhum projeto do Compose, em geral sobras de builds e jobs de CI (${bytes(dados?.docker?.sobras_bytes ?? 0)}). Os containers dos seus projetos ficam, ligados ou parados.`,
                    )}>{limpando === 'sobras' ? 'Apagando' : `Sobras de builds (${dados.docker.sobras})`}</button
                >
                <button
                  class="botao"
                  disabled={!!limpando || containers.quantidade === containers.ativos}
                  onclick={() =>
                    limparDocker(
                      'parados',
                      'Apagar todos os containers parados?',
                      'Apaga todo container parado, inclusive os de projetos do Compose. Volumes e imagens ficam, mas um projeto parado precisa de docker compose up para recriar os containers dele; o botão Ligar do app deixa de servir para ele.',
                    )}>{limpando === 'parados' ? 'Apagando' : 'Todos os parados'}</button
                >
              </td>
            </tr>
          {/if}
          {#if volumes}
            <tr>
              <th scope="row">{NOMES[volumes.tipo]} <small>{volumes.quantidade}, {dados.docker.volumes_sem_uso.length} sem uso</small></th>
              <td class="numero">{bytes(volumes.total_bytes)}</td>
              <td class="numero">{bytes(volumes.recuperavel_bytes)}</td>
              <td class="acao">
                <button class="botao" aria-expanded={volumesAbertos} onclick={() => (volumesAbertos = !volumesAbertos)}>
                  {volumesAbertos ? 'Fechar lista' : 'Escolher volumes'}
                </button>
              </td>
            </tr>
            {#if volumesAbertos}
              <tr class="lista">
                <td colspan="4">
                  <p class="nota">
                    Volume guarda dados, como o banco de um projeto parado. Só aparecem os que nenhum container usa agora; o que
                    estiver num volume apagado se perde de vez.
                  </p>
                  <ul>
                    {#each dados.docker.volumes_sem_uso.slice(0, 60) as v (v.nome)}
                      <li>
                        <label><input type="checkbox" bind:checked={volumesMarcados[v.nome]} /> <code>{v.nome}</code></label>
                        <span class="numero">{bytes(v.bytes)}</span>
                      </li>
                    {/each}
                  </ul>
                  <button
                    class="botao perigo"
                    disabled={!!limpando || volumesEscolhidos.length === 0}
                    onclick={() =>
                      limparDocker(
                        'volumes',
                        `Apagar ${volumesEscolhidos.length} volume(s)?`,
                        `Apaga ${volumesEscolhidos.map((v) => v.nome).join(', ')}, somando ${bytes(volumesEscolhidos.reduce((s, v) => s + v.bytes, 0))}. Os dados deles se perdem de vez.`,
                        volumesEscolhidos.map((v) => v.nome),
                      )}>{limpando === 'volumes' ? 'Apagando' : 'Apagar os marcados'}</button
                  >
                </td>
              </tr>
            {/if}
          {/if}
          {#if build}
            <tr>
              <th scope="row">{NOMES[build.tipo]} <small>{build.quantidade} itens</small></th>
              <td class="numero">{bytes(build.total_bytes)}</td>
              <td class="numero">{bytes(build.recuperavel_bytes)}</td>
              <td class="acao">
                <button
                  class="botao"
                  disabled={!!limpando || build.total_bytes === 0}
                  onclick={() =>
                    limparDocker(
                      'cache_de_build',
                      'Apagar o cache de build?',
                      'Apaga o cache que nenhum build está usando agora. O próximo build de cada imagem refaz as etapas e demora mais na primeira vez.',
                    )}>{limpando === 'cache_de_build' ? 'Apagando' : 'Apagar cache'}</button
                >
              </td>
            </tr>
          {/if}
        </tbody>
      </table>
      {#if dados.docker.projetos.length}
        <h3>Por projeto do Compose</h3>
        <p class="nota">
          Imagem entra no projeto só quando nenhum outro a usa, e só com a parte que ela não divide com outras imagens; o resto
          fica na linha Imagens acima. Volume em uso é de um container do projeto, ligado ou parado, e não se apaga daqui.
        </p>
        <table>
          <thead>
            <tr>
              <th scope="col">Projeto</th>
              <th scope="col" class="numero">Containers</th>
              <th scope="col" class="numero">Imagens</th>
              <th scope="col" class="numero">Volumes</th>
              <th scope="col" class="numero">Total</th>
              <th scope="col"><span class="visualmente-oculto">Ação</span></th>
            </tr>
          </thead>
          <tbody>
            {#each dados.docker.projetos as p (p.nome)}
              {@const semUso = p.volumes.filter((v) => !v.em_uso)}
              {@const volumesBytes = p.volumes.reduce((s, v) => s + v.bytes, 0)}
              <tr>
                <th scope="row">{p.nome} <small>{p.containers} container(s){p.parados ? `, ${p.parados} parado(s)` : ''}</small></th>
                <td class="numero">{bytes(p.containers_bytes)}</td>
                <td class="numero">{bytes(p.imagens_bytes)}</td>
                <td class="numero">{bytes(volumesBytes)}</td>
                <td class="numero">{bytes(p.containers_bytes + p.imagens_bytes + volumesBytes)}</td>
                <td class="acao">
                  {#if p.parados}
                    <button class="botao" disabled={!!limpando} onclick={() => apagarParadosDoProjeto(p)}
                      >{limpando === `parados:${p.nome}` ? 'Apagando' : `Parados (${p.parados})`}</button
                    >
                  {/if}
                  {#if semUso.length}
                    <button class="botao" disabled={!!limpando} onclick={() => apagarVolumesDoProjeto(p.nome, semUso)}
                      >{limpando === 'volumes' ? 'Apagando' : `Volumes sem uso (${semUso.length})`}</button
                    >
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {:else}
      <p class="nota">Lendo.</p>
    {/if}
  </section>
  {/if}

  <section class="cartao" aria-labelledby="titulo-limpeza">
    <div class="cabeca">
      <h2 id="titulo-limpeza">Limpeza programada</h2>
      <span>Uma vez por semana, com o app aberto</span>
    </div>
    {#if dados}
      {#if temDocker}
        <label class="opcao">
          <input type="checkbox" checked={dados.limpeza.cache_de_build} onchange={(e) => mudarLimpeza({ cache_de_build: e.currentTarget.checked })} />
          Apagar o cache de build do Docker que nenhum build usa
        </label>
        <label class="opcao">
          <input
            type="checkbox"
            checked={dados.limpeza.sobras_de_containers}
            onchange={(e) => mudarLimpeza({ sobras_de_containers: e.currentTarget.checked })}
          />
          Apagar containers parados do Docker que não são de nenhum projeto do Compose
        </label>
      {/if}
      <label class="opcao">
        Apagar do /tmp de cada distro ligada o que não muda há
        <select
          value={String(dados.limpeza.tmp_dias ?? '')}
          onchange={(e) => mudarLimpeza({ tmp_dias: e.currentTarget.value ? Number(e.currentTarget.value) : null })}
        >
          <option value="">nunca</option>
          <option value="7">7 dias</option>
          <option value="14">14 dias</option>
          <option value="30">30 dias</option>
        </select>
      </label>
      <p class="nota">
        Pasta montada por container rodando fica de fora. Cada limpeza vai para a Atividade com o quanto liberou.
        {dados.limpeza.ultima_ms ? `A última foi em ${data.format(dados.limpeza.ultima_ms)}.` : 'Nenhuma rodou ainda.'}
      </p>
    {/if}
  </section>
</div>

<style>
  .rolagem {
    flex: 1;
    overflow: auto;
    display: grid;
    align-content: start;
    gap: 12px;
    padding: 0 20px 20px;
  }

  .topo {
    display: flex;
    justify-content: flex-end;
  }

  .cartao {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    border: 1px solid var(--borda);
    border-radius: 8px;
    background: var(--camada);
  }

  .cabeca {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }

  .cabeca span,
  .nota {
    color: var(--texto-2);
    font-size: 12px;
  }

  .nota {
    margin: 0;
    max-width: 90ch;
  }

  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }

  h3 {
    margin: 8px 0 0;
    font-size: 14px;
    font-weight: 600;
  }

  .resultado {
    margin: 0;
    padding: 8px 12px;
    border-left: 3px solid var(--ok);
    border-radius: var(--raio);
    background: var(--camada-forte);
    font-size: 13px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  th,
  td {
    padding: 6px 10px;
    text-align: left;
    border-bottom: 1px solid var(--borda);
    vertical-align: middle;
  }

  thead th {
    color: var(--texto-2);
    font-weight: 400;
    font-size: 12px;
  }

  tbody th {
    font-weight: 600;
    white-space: nowrap;
  }

  small {
    margin-left: 6px;
    color: var(--texto-2);
    font-weight: 400;
  }

  .numero {
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  /* Com max-width 0, a célula ocupa o que sobra da tabela e corta o caminho longo, em vez de
     alargar a tabela. */
  .caminho {
    max-width: 0;
    width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--texto-2);
  }

  .acao {
    width: 1%;
    white-space: nowrap;
    text-align: right;
  }

  .acao .botao + .botao {
    margin-left: 6px;
  }

  .nota-linha td {
    color: var(--texto-2);
    font-size: 12px;
  }

  .lista ul,
  .raiz {
    display: grid;
    gap: 2px;
    margin: 6px 0;
    padding: 0;
    list-style: none;
    font-size: 13px;
  }

  .lista li,
  .raiz li {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .lista ul {
    max-height: 280px;
    overflow: auto;
  }

  .bloqueada td {
    color: var(--texto-2);
  }

  .analisar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 13px;
  }

  .analisar select,
  .opcao select {
    height: 30px;
    margin-left: 6px;
    padding: 0 6px;
    border: 1px solid var(--borda);
    border-radius: var(--raio);
    background: var(--camada-forte);
  }

  .opcao {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
</style>
