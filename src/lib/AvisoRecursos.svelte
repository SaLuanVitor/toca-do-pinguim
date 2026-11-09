<script lang="ts">
  // Aviso que vale em qualquer página. O de RAM acabando fica só em Desempenho > Memória, e a
  // entrada Desempenho no menu ganha um sinal enquanto ele estiver ativo. Um aviso por vez, do
  // mais grave ao mais cedo: WSL travado, Docker derrubado, Docker sem resposta, VM sob pressão.
  import { api } from './api';
  import { baixarPrioridadeDe, chaveDeAlivio, desligarWsl, ocupadas, pausarConteinerDe } from './acoes.svelte';
  import { estado } from './estado.svelte';
  import type { Pesado } from './tipos';

  const hora = new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' });
  const GIB = 1024 ** 3;
  let abrindo = $state(false);

  $effect(() => {
    if (estado.docker.ligado) estado.dockerDerrubadoMs = null;
  });

  const numero = (v: number) => v.toFixed(1).replace('.', ',');

  /** "2 postgres em docker-desktop: 1,9 núcleos, 3,4 GB". */
  function descrever(p: Pesado): string {
    const partes: string[] = [];
    if (p.cpu_nucleos >= 0.5) partes.push(`${numero(p.cpu_nucleos)} ${p.cpu_nucleos < 1.5 ? 'núcleo' : 'núcleos'}`);
    if (p.rss >= GIB) partes.push(`${numero(p.rss / GIB)} GB`);
    if (p.disco_bps >= 5 * 1024 ** 2) partes.push(`${numero(p.disco_bps / 1024 ** 2)} MB/s de disco`);
    if (p.esperando > 0) partes.push(p.esperando === p.quantos ? 'esperando disco' : `${p.esperando} esperando disco`);
    return `${p.quantos > 1 ? `${p.quantos} ` : ''}${p.nome} em ${p.distro}: ${partes.join(', ')}`;
  }

  /** Nome do container dono do grupo, pela lista do Docker; vazio se ele não aparece lá. */
  function nomeDoConteiner(p: Pesado): string {
    return estado.docker.conteineres.find((c) => c.id === p.conteiner)?.nome ?? '';
  }

  /** CPU acima de meio núcleo justifica baixar a prioridade; disco e espera, não: o WSL não tem prioridade de disco. */
  const pesaNaCpu = (p: Pesado) => p.cpu_nucleos >= 0.5 && p.pids.length > 0;

  async function abrirDocker() {
    abrindo = true;
    try {
      await api.abrirDockerDesktop();
    } catch (e) {
      estado.avisar(String(e));
    } finally {
      abrindo = false;
    }
  }
</script>

{#snippet pesados(lista: Pesado[], aliviar: boolean)}
  {#if lista.length}
    <ul class="pesados" aria-label="Quem mais pesa">
      {#each lista as p (`${p.distro}/${p.conteiner ?? ''}/${p.nome}`)}
        {@const ocupado = ocupadas[chaveDeAlivio(p)]}
        {@const conteiner = nomeDoConteiner(p)}
        <li>
          <span>{descrever(p)}{conteiner ? `, no container ${conteiner}` : ''}</span>
          {#if aliviar && pesaNaCpu(p)}
            <button class="botao pequeno" disabled={!!ocupado} onclick={() => baixarPrioridadeDe(p)}>
              {ocupado === 'Baixando' ? 'Baixando' : 'Baixar prioridade'}
            </button>
          {/if}
          {#if aliviar && conteiner}
            <button class="botao pequeno" disabled={!!ocupado} onclick={() => pausarConteinerDe(p, conteiner)}>
              {ocupado === 'Pausando' ? 'Pausando' : 'Pausar o container'}
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
{/snippet}

{#if estado.saude}
  <div class="aviso perigo" role="alert">
    <span>
      <strong>O WSL não está abrindo sessões novas</strong> desde {hora.format(estado.saude.desde_ms)}{estado.saude.alvo
        ? ` em ${estado.saude.alvo}`
        : ''}: {estado.saude.motivo}. O que já está rodando, como containers e terminais abertos, continua, mas o app não
      consegue ler as distros. Forçar o desligamento o destrava; todas as distros e os containers param.
    </span>
    <button class="botao primario" onclick={desligarWsl} disabled={!!ocupadas.WSL}>{ocupadas.WSL ?? 'Forçar o desligamento'}</button>
  </div>
{:else if estado.dockerDerrubadoMs && !estado.docker.ligado}
  <div class="aviso" role="status">
    <span>
      <strong>O Docker Desktop caiu junto com o WSL</strong> às {hora.format(estado.dockerDerrubadoMs)}. Se ele estava aberto,
      costuma religar sozinho em alguns segundos; se não voltar, abra-o.
    </span>
    <button class="botao" onclick={abrirDocker} disabled={abrindo}>{abrindo ? 'Abrindo' : 'Abrir o Docker Desktop'}</button>
    <button class="botao sutil" onclick={() => (estado.dockerDerrubadoMs = null)}>Dispensar</button>
  </div>
{:else if estado.docker.sem_resposta_desde_ms}
  <div class="aviso" role="alert">
    <span>
      <strong>O Docker não responde</strong> desde {hora.format(estado.docker.sem_resposta_desde_ms)}. O WSL pode travar em
      seguida: pare os projetos mais pesados ou feche o Docker Desktop enquanto ainda dá. Se o WSL travar, o app oferece o
      desligamento forçado.
      {@render pesados(estado.docker.pesados, false)}
    </span>
    <button class="botao" onclick={() => (estado.pagina = 'processos')}>Ver processos</button>
  </div>
{:else if estado.pressao}
  <div class="aviso" role="alert">
    <span>
      <strong>A VM do WSL está sobrecarregada</strong> desde {hora.format(estado.pressao.desde_ms)}:
      {#if estado.pressao.cpu_vm / 90 >= Math.max(estado.pressao.disco_full, estado.pressao.memoria_full) / 10}
        a CPU dela está em {Math.round(estado.pressao.cpu_vm)}%.
      {:else}
        no último minuto, ela ficou parada esperando {estado.pressao.disco_full >= estado.pressao.memoria_full ? 'disco' : 'memória'}
        {Math.round(Math.max(estado.pressao.disco_full, estado.pressao.memoria_full))}% do tempo.
      {/if}
      Se continuar, o Docker e o WSL podem travar: limite a CPU do container que mais pesa, em Desempenho &gt; Processamento,
      baixe a prioridade de um processo ou pare o que não precisa agora.
      {@render pesados(estado.pressao.pesados, true)}
    </span>
    <button class="botao" onclick={() => (estado.pagina = 'processos')}>Ver processos</button>
  </div>
{/if}

<style>
  .aviso {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 12px 20px 0;
    padding: 8px 8px 8px 12px;
    border: 1px solid var(--borda);
    border-left: 3px solid rgb(var(--calor));
    border-radius: var(--raio);
    background: rgba(var(--calor), 0.12);
    font-size: 13px;
  }

  .aviso.perigo {
    border-left-color: var(--perigo);
    background: var(--camada-forte);
  }

  .aviso span {
    flex: 1;
    min-width: 200px;
  }

  .pesados {
    margin: 6px 0 0;
    padding-left: 18px;
  }

  .pesados li {
    margin-top: 4px;
  }

  .pesados li span {
    margin-right: 8px;
  }

  .botao.pequeno {
    height: 24px;
    margin-right: 4px;
    font-size: 12px;
  }
</style>
