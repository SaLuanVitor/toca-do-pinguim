// Explicações que aparecem em mais de uma tela.

export const TEXTO_DEVOLVER_CACHE =
  "O Linux guarda na RAM cópias dos arquivos que leu, para não precisar ler do disco de novo. Dentro da VM do WSL quase nunca falta memória, então ele não as solta, e o Windows conta essa RAM como ocupada pela VM. Devolver joga essas cópias fora e a RAM volta ao Windows. Nada se apaga do disco: a VM só relê o que precisar, um pouco mais devagar na primeira vez. O cache volta a crescer com o uso; para a VM ocupar menos de vez, baixe o teto de memória.";
