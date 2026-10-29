// Imprime as notas de uma versão a partir do CHANGELOG.md, para o corpo da release.
//
//   node scripts/notas-da-versao.mjs v1.2.3

import { readFileSync } from 'node:fs';

const tag = process.argv[2];
if (!tag?.startsWith('v')) throw new Error('uso: node scripts/notas-da-versao.mjs v<versão>');
const versao = tag.slice(1);

const linhas = readFileSync('CHANGELOG.md', 'utf8').replace(/\r\n/g, '\n').split('\n');
const inicio = linhas.findIndex((l) => l.startsWith(`## [${versao}]`));
if (inicio < 0) throw new Error(`CHANGELOG.md não tem a seção ## [${versao}]`);
const fim = linhas.findIndex((l, i) => i > inicio && (l.startsWith('## [') || /^\[[^\]]+\]: /.test(l)));
const secao = linhas.slice(inicio + 1, fim < 0 ? undefined : fim).join('\n').trim();

console.log(`${secao}

## Arquivos

| Arquivo | Para quê |
|---|---|
| \`toca-do-pinguim_${versao}_x64-setup.exe\` | instalador |
| \`toca-do-pinguim_${versao}_x64-portable.zip\` | sem instalação: extraia e abra \`toca-do-pinguim.exe\`. Os dados ficam em \`%APPDATA%\\com.luan.tocadopinguim\` do mesmo jeito |
| \`SHA256SUMS.txt\` | para conferir o download: \`Get-FileHash <arquivo>\` no PowerShell |
| \`latest.json\` | lido pelo app instalado para avisar e instalar a versão nova |
| \`THIRD-PARTY-NOTICES.md\` | licenças das bibliotecas de terceiros incluídas no app |

Requer Windows 10 ou 11 x64 com WSL 2. Ainda sem assinatura digital: o SmartScreen avisa na
primeira execução, e o caminho é "Mais informações" e depois "Executar assim mesmo".`);
