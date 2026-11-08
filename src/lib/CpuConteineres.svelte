<script lang="ts">
  // CPU de cada container ligado e o limite dele, com o ajuste na hora (`docker update --cpus`).
  // O limite fica na configuração do container: sobrevive a reiniciar, mas some se o Compose o
  // recriar, e por isso a nota aponta o `cpus:` do compose.
  import { api } from './api';
  import { estado } from './estado.svelte';

  let { cpus }: { cpus: number } = $props();

  let editando = $state<string | null>(null);
  let nucleos = $state(1);
  let ocupado = $state<string | null>(null);

  const numero = (v: number, casas = 1) => v.toLocaleString('pt-BR', { minimumFractionDigits: casas, maximumFractionDigits: casas });
  /** Limite igual ao total da VM é o "sem limite" que o app grava: o Docker não aceita tirar de outro jeito. */
  const temLimite = (limite: number) => limite > 0 && limite < cpus;

  const ligados = $derived(
    estado.docker.conteineres.filter((c) => c.estado === 'running').toSorted((a, b) => b.cpu_nucleos - a.cpu_nucleos),
  );
  const semLimite = $derived(ligados.filter((c) => !temLimite(c.limite_cpus)).length);
  const valido = $derived(Number.isFinite(nucleos) && nucleos >= 0.1 && nucleos <= cpus);

  function editar(id: string, limite: number) {
    editando = id;
    nucleos = temLimite(limite) ? limite : Math.min(2, cpus);
  }

  async function aplicar(id: string, nome: string, limite: number | null) {
    ocupado = id;
    try {
      await api.limitarCpuConteiner(id, nome, limite);
      editando = null;
    } catch (e) {
      estado.avisar(String(e));
    } finally {
      ocupado = null;
    }
  }
</script>

{#if estado.docker.ligado && ligados.length}
  <section class="cartao largo" aria-labelledby="titulo-cpu-conteineres">
    <div class="cabeca">
      <h2 id="titulo-cpu-conteineres">CPU dos containers</h2>
      <span>1,0 é um núcleo inteiro · {semLimite} de {ligados.length} sem limite</span>
    </div>
    <table>
      <thead>
        <tr>
          <th scope="col">Container</th>
          <th scope="col" class="numero">CPU</th>
          <th scope="col">Limite</th>
          <th scope="col"><span class="visualmente-oculto">Ação</span></th>
        </tr>
      </thead>
      <tbody>
        {#each ligados as c (c.id)}
          <tr>
            <th scope="row">
              {c.nome}
              {#if c.projeto}<span class="projeto">{c.projeto}</span>{/if}
            </th>
            <td class="numero">{numero(c.cpu_nucleos, 2)}</td>
            <td>{temLimite(c.limite_cpus) ? `${numero(c.limite_cpus)} ${c.limite_cpus < 1.5 ? 'núcleo' : 'núcleos'}` : 'sem limite'}</td>
            <td class="acoes">
              {#if editando === c.id}
                <label>
                  <span class="visualmente-oculto">Núcleos para {c.nome}</span>
                  <input type="number" min="0.1" max={cpus} step="0.5" bind:value={nucleos} aria-invalid={!valido} />
                </label>
                <button class="botao primario" disabled={!valido || ocupado === c.id} onclick={() => aplicar(c.id, c.nome, nucleos)}>
                  {ocupado === c.id ? 'Aplicando' : 'Aplicar'}
                </button>
                <button class="botao sutil" onclick={() => (editando = null)}>Cancelar</button>
              {:else}
                <button class="botao" disabled={!!ocupado} onclick={() => editar(c.id, c.limite_cpus)}>
                  {temLimite(c.limite_cpus) ? 'Mudar limite' : 'Limitar'}
                </button>
                {#if temLimite(c.limite_cpus)}
                  <button class="botao sutil" disabled={!!ocupado} onclick={() => aplicar(c.id, c.nome, null)}>
                    {ocupado === c.id ? 'Tirando' : 'Tirar limite'}
                  </button>
                {/if}
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="nota">
      O limite vale na hora, sem reiniciar o container, e continua depois de um reinício. Se o Compose recriar o
      container, ele some: para ficar de vez, ponha <code>cpus:</code> no serviço do compose.
    </p>
  </section>
{/if}

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

  .cabeca span,
  .nota,
  .projeto {
    color: var(--texto-2);
    font-size: 12px;
  }

  h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  th,
  td {
    height: 40px;
    padding: 0 10px;
    text-align: left;
    border-bottom: 1px solid var(--borda);
  }

  thead th {
    color: var(--texto-2);
    font-weight: 400;
    font-size: 12px;
  }

  tbody th {
    font-weight: 600;
  }

  .projeto {
    margin-left: 8px;
    font-weight: 400;
  }

  .numero {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .acoes {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
  }

  .acoes input {
    width: 72px;
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--borda);
    border-bottom-color: var(--borda-forte);
    border-radius: var(--raio);
    background: var(--camada-forte);
  }

  .acoes input[aria-invalid='true'] {
    border-bottom: 2px solid var(--perigo);
  }

  .acoes .botao {
    height: 28px;
  }

  .nota {
    margin: 0;
  }
</style>
