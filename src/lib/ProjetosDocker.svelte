<script lang="ts">
  import { ligarProjeto, ocupadas, pararProjeto, reiniciarComProblema } from './acoes.svelte';
  import { estado } from './estado.svelte';
  import { bytes } from './formato';
  import { agrupar, comProblema, pertoDoLimite, semResposta } from './projetos';

  const docker = $derived(estado.docker);
  const projetos = $derived(agrupar(docker.conteineres));
  const total = $derived(projetos.reduce((s, p) => s + p.memoria, 0));
  const rodando = $derived(projetos.filter((p) => p.rodando > 0).length);
</script>

<section class="cartao" aria-labelledby="titulo-docker">
  <div class="cabeca">
    <h2 id="titulo-docker">RAM do Docker por projeto</h2>
    {#if docker.ligado && !docker.erro}
      <span>{bytes(total)} de RAM em {rodando} projeto(s) ligado(s), lido a cada 15 s</span>
    {/if}
  </div>

  {#if !docker.ligado}
    <p class="nota">O Docker Desktop não está ligado.</p>
  {:else if docker.erro}
    <p class="nota">Não foi possível ler o Docker: {docker.erro}</p>
  {:else if projetos.length === 0}
    <p class="nota">Nenhum container em execução.</p>
  {:else}
    <table>
      <thead>
        <tr>
          <th scope="col">Projeto</th>
          <th scope="col" class="numero">Containers ligados</th>
          <th scope="col" class="numero">RAM</th>
          <th scope="col">Situação</th>
          <th scope="col"><span class="visualmente-oculto">Ação</span></th>
        </tr>
      </thead>
      <tbody>
        {#each projetos as p (p.chave)}
          {@const ocupado = ocupadas[`docker:${p.chave}`]}
          {@const semResp = p.conteineres.filter(semResposta)}
          {@const perto = p.conteineres.filter(pertoDoLimite)}
          <tr class:parado={p.rodando === 0}>
            <th scope="row">
              {p.nome}
              {#if p.avulso}<small>container avulso</small>{/if}
            </th>
            <td class="numero">{p.rodando} de {p.conteineres.length}</td>
            <td class="numero">{p.rodando ? bytes(p.memoria) : ''}</td>
            <td>
              {#if semResp.length}
                <span class="ciclo">
                  {semResp.map((c) => c.nome).join(', ')} não {semResp.length === 1 ? 'responde' : 'respondem'} ao healthcheck
                </span>
              {/if}
              {#if perto.length}
                <span class="ciclo">
                  {perto.map((c) => `${c.nome} usa ${bytes(c.memoria_bytes)} de ${bytes(c.limite_bytes)}`).join(', ')}
                </span>
              {/if}
              {#if semResp.length || perto.length}{:else if p.emCiclo.length}
                <span class="ciclo">
                  {#each p.emCiclo as c, i (c.id)}{i ? ', ' : ''}{c.nome}{/each}
                  {p.emCiclo.length === 1 ? 'reinicia' : 'reiniciam'} sem parar ({p.emCiclo
                    .map((c) => `${c.reinicios} reinícios`)
                    .join(', ')})
                </span>
              {:else if p.rodando === 0}
                Parado
              {:else if p.rodando < p.conteineres.length}
                Em parte ligado
              {:else}
                Ligado
              {/if}
            </td>
            <td class="acao">
              {#if p.conteineres.some(comProblema)}
                <button class="botao" disabled={!!ocupado} onclick={() => reiniciarComProblema(p)}>Reiniciar</button>
              {/if}
              {#if p.rodando > 0}
                <button class="botao" disabled={!!ocupado} onclick={() => pararProjeto(p)}>{ocupado ?? 'Parar'}</button>
              {:else}
                <button class="botao" disabled={!!ocupado} onclick={() => ligarProjeto(p)}>{ocupado ?? 'Ligar'}</button>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="nota">
      Parar aqui é o mesmo que <code>docker stop</code>: o container fica parado até você ligar de novo, e os que usam
      <code>restart: always</code> também voltam quando o Docker reinicia.
    </p>
  {/if}
</section>

<style>
  .cartao {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    gap: 12px;
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
    height: 34px;
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

  small {
    margin-left: 6px;
    color: var(--texto-2);
    font-weight: 400;
  }

  .numero {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .parado th,
  .parado td:not(.acao) {
    color: var(--texto-2);
  }

  .ciclo {
    display: block;
    color: var(--perigo);
  }

  .acao {
    width: 1%;
    white-space: nowrap;
    text-align: right;
  }
</style>
