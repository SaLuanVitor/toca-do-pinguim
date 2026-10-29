<script lang="ts">
  // Gráfico de linha de 0 a 100 no formato do Gerenciador de Tarefas: grade fina,
  // janela de 60 pontos e o valor mais novo na borda direita.
  let {
    valores,
    cor,
    rotulo,
    pontos = 60,
  }: { valores: number[]; cor: string; rotulo: string; pontos?: number } = $props();

  let canvas: HTMLCanvasElement;
  let largura = $state(0);
  let altura = $state(0);

  function corCss(nome: string): string {
    return getComputedStyle(canvas).getPropertyValue(nome).trim() || '#888';
  }

  $effect(() => {
    const dpr = window.devicePixelRatio || 1;
    const w = largura;
    const h = altura;
    if (!w || !h) return;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    const g = canvas.getContext('2d');
    if (!g) return;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);

    const linha = corCss(cor);
    g.strokeStyle = corCss('--borda');
    g.lineWidth = 1;
    g.beginPath();
    for (let i = 1; i < 10; i++) {
      const y = Math.round((h * i) / 10) + 0.5;
      g.moveTo(0, y);
      g.lineTo(w, y);
    }
    for (let i = 1; i < 12; i++) {
      const x = Math.round((w * i) / 12) + 0.5;
      g.moveTo(x, 0);
      g.lineTo(x, h);
    }
    g.stroke();

    if (valores.length < 2) return;
    const passo = w / (pontos - 1);
    const inicio = w - (valores.length - 1) * passo;
    const y = (v: number) => h - (Math.min(100, Math.max(0, v)) / 100) * (h - 2) - 1;

    g.beginPath();
    valores.forEach((v, i) => (i === 0 ? g.moveTo(inicio, y(v)) : g.lineTo(inicio + i * passo, y(v))));
    g.lineTo(w, h);
    g.lineTo(inicio, h);
    g.closePath();
    g.globalAlpha = 0.18;
    g.fillStyle = linha;
    g.fill();

    g.globalAlpha = 1;
    g.beginPath();
    valores.forEach((v, i) => (i === 0 ? g.moveTo(inicio, y(v)) : g.lineTo(inicio + i * passo, y(v))));
    g.strokeStyle = linha;
    g.lineWidth = 1.5;
    g.stroke();
  });
</script>

<div
  class="quadro"
  role="img"
  aria-label={rotulo}
  style:--linha="var({cor})"
  bind:clientWidth={largura}
  bind:clientHeight={altura}
>
  <canvas bind:this={canvas} aria-hidden="true" style:width="{largura}px" style:height="{altura}px"></canvas>
</div>

<style>
  .quadro {
    position: relative;
    min-height: 140px;
    height: 100%;
    border: 1px solid var(--linha);
    border-radius: 2px;
    overflow: hidden;
  }

  canvas {
    position: absolute;
    inset: 0;
    display: block;
  }
</style>
