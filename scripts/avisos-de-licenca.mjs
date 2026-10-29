// Monta o THIRD-PARTY-NOTICES.md: a parte de Rust vem do cargo-about, e esta script
// acrescenta os pacotes npm que entram no front empacotado no executável.
//
//   cargo about generate about.hbs -o licencas-rust.md
//   node scripts/avisos-de-licenca.mjs licencas-rust.md THIRD-PARTY-NOTICES.md

import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const [rust, saida] = process.argv.slice(2);
if (!rust || !saida) throw new Error('uso: node scripts/avisos-de-licenca.mjs <licencas-rust.md> <saida.md>');

// O que o Vite empacota no app: o runtime do Svelte e do SvelteKit e a API do Tauri, com o
// que eles importam. Ferramentas de build (Vite, TypeScript) não vão para o executável.
const RAIZES = ['@tauri-apps/api', 'svelte', '@sveltejs/kit'];

function achar(nome, de) {
  for (let pasta = de; ; pasta = dirname(pasta)) {
    const candidato = join(pasta, 'node_modules', nome, 'package.json');
    if (existsSync(candidato)) return candidato;
    if (dirname(pasta) === pasta) return null;
  }
}

const pacotes = new Map();
const fila = RAIZES.map((nome) => [nome, process.cwd()]);
while (fila.length) {
  const [nome, de] = fila.shift();
  const arquivo = achar(nome, de);
  if (!arquivo) continue;
  const pkg = JSON.parse(readFileSync(arquivo, 'utf8'));
  const chave = `${pkg.name}@${pkg.version}`;
  if (pacotes.has(chave)) continue;
  const pasta = dirname(arquivo);
  const licenca = readdirSync(pasta).find((f) => /^(licen[cs]e|copying)(\..*)?$/i.test(f));
  pacotes.set(chave, {
    nome: pkg.name,
    versao: pkg.version,
    licenca: typeof pkg.license === 'string' ? pkg.license : (pkg.license?.type ?? 'não declarada'),
    texto: licenca ? readFileSync(join(pasta, licenca), 'utf8').trim() : null,
  });
  for (const dep of Object.keys(pkg.dependencies ?? {})) fila.push([dep, pasta]);
}

const npm = [...pacotes.values()]
  .sort((a, b) => a.nome.localeCompare(b.nome))
  .map(
    (p) =>
      `### ${p.nome} ${p.versao}\n\nLicença: ${p.licenca}\n\n` +
      (p.texto ? '```text\n' + p.texto + '\n```\n' : 'O pacote não traz o texto da licença.\n'),
  )
  .join('\n');

writeFileSync(
  saida,
  `# Avisos de licença de terceiros

A Toca do Pinguim é distribuída sob a licença MIT (veja LICENSE) e inclui o código abaixo, de
terceiros, cada um sob a própria licença. O código-fonte das dependências em Rust, inclusive
as sob a MPL-2.0, está no crates.io, na versão listada; o dos pacotes npm, no registro npm.

${readFileSync(rust, 'utf8').trim()}

## Pacotes npm empacotados na interface

${npm}`,
);
console.log(`${saida}: ${pacotes.size} pacotes npm e a parte de Rust do cargo-about.`);
