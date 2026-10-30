<script lang="ts">
  // Traços de 16 px no espírito dos ícones Fluent; decorativos, o texto ao lado nomeia.
  const CAMINHOS = {
    menu: 'M2 4h12M2 8h12M2 12h12',
    processos: 'M2 3.5h12M2 8h12M2 12.5h12M2 3.5h.01M2 8h.01M2 12.5h.01',
    processador: 'M4.5 4.5h7v7h-7zM6.5 6.5h3v3h-3zM6.5 2v2.5M9.5 2v2.5M6.5 11.5V14M9.5 11.5V14M2 6.5h2.5M2 9.5h2.5M11.5 6.5H14M11.5 9.5H14',
    memoria: 'M1.5 5h13v5.5h-13zM4 7.5h1.5M7.25 7.5h1.5M10.5 7.5H12M3 10.5V12.5M6 10.5V12.5M10 10.5V12.5M13 10.5V12.5',
    distros: 'M3 2.5h10v4H3zM3 9.5h10v4H3zM5 4.5h.01M5 11.5h.01',
    portas: 'M5.5 2v3.5M10.5 2v3.5M3.5 5.5h9v2.5a4.5 4.5 0 0 1-9 0zM8 12.5V15',
    atividade: 'M8 14.5a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13zM8 4.5V8l2.5 1.5',
    busca: 'M7 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM14 14l-3.5-3.5',
    chevron: 'M6 4l4 4-4 4',
    parar: 'M4 4l8 8M12 4l-8 8',
    play: 'M5 3.5v9l7-4.5z',
    reiniciar: 'M13 8a5 5 0 1 1-1.5-3.6M13 2.5V5h-2.5',
    energia: 'M8 2v6M4.8 4.2a5 5 0 1 0 6.4 0',
    fechar: 'M4 4l8 8M12 4l-8 8',
    configuracoes: 'M8 5.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5zM8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4',
    log: 'M4 1.5h5.5L12.5 4.5v10h-8.5zM9.5 1.5v3h3M6 8h4.5M6 10.5h4.5M6 13h2.5',
    pasta: 'M1.5 3.5h4.5l1.5 1.5h7v8.5h-13z',
    disco: 'M2 9.5h12v4H2zM2 9.5l2-6.5h8l2 6.5M11.5 11.5h.01',
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
