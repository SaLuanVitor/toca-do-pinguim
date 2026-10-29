<script lang="ts">
  // Traços de 16 px no espírito dos ícones Fluent; decorativos, o texto ao lado nomeia.
  const CAMINHOS = {
    menu: 'M2 4h12M2 8h12M2 12h12',
    processos: 'M2 3.5h12M2 8h12M2 12.5h12M2 3.5h.01M2 8h.01M2 12.5h.01',
    desempenho: 'M2 13.5h12M3 11l3-4 3 2 4-6',
    distros: 'M3 2.5h10v4H3zM3 9.5h10v4H3zM5 4.5h.01M5 11.5h.01',
    busca: 'M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM14 14l-3.5-3.5',
    chevron: 'M6 4l4 4-4 4',
    parar: 'M4 4l8 8M12 4l-8 8',
    play: 'M5 3.5v9l7-4.5z',
    reiniciar: 'M13 8a5 5 0 1 1-1.5-3.6M13 2.5V5h-2.5',
    energia: 'M8 2v6M4.8 4.2a5 5 0 1 0 6.4 0',
    fechar: 'M4 4l8 8M12 4l-8 8',
  } as const;

  let { nome, tamanho = 16 }: { nome: keyof typeof CAMINHOS; tamanho?: number } = $props();
</script>

<svg
  width={tamanho}
  height={tamanho}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.3"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  <path d={CAMINHOS[nome]} />
</svg>
