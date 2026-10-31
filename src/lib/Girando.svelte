<script lang="ts">
  // Indicador de leitura em andamento, ao lado do texto que diz o que está sendo lido. É só
  // visual: o texto vizinho, num role="status", é o que o leitor de tela anuncia.
  let { tamanho = 14 }: { tamanho?: number } = $props();
</script>

<span class="girando" style:width="{tamanho}px" style:height="{tamanho}px" aria-hidden="true"></span>

<style>
  .girando {
    display: inline-block;
    flex: none;
    vertical-align: -2px;
    border: 2px solid var(--borda-forte);
    border-top-color: var(--destaque);
    border-radius: 50%;
    animation: girar 0.8s linear infinite;
  }

  @keyframes girar {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .girando {
      animation-duration: 2.4s;
    }
  }
</style>
