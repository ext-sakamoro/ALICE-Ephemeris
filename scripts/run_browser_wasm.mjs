// Run every `check_*` export of the browser build (browser/, wasm32-unknown-unknown)
// under node. Fails if a check traps, if no check ran, or if the number that ran
// differs from `check_count()` or from the number of #[test] functions in the
// test files the browser crate includes.
// usage: node scripts/run_browser_wasm.mjs <path to .wasm>
import fs from 'node:fs';

const wasmPath = process.argv[2];
const { instance } = await WebAssembly.instantiate(fs.readFileSync(wasmPath), {});
const ex = instance.exports;

const lib = fs.readFileSync(new URL('../browser/src/lib.rs', import.meta.url), 'utf8');
let tests = 0;
for (const m of lib.matchAll(/#\[path = "\.\.\/\.\.\/(tests\/[a-z_0-9]+\.rs)"\]/g)) {
  const src = fs.readFileSync(new URL('../' + m[1], import.meta.url), 'utf8');
  tests += (src.match(/^#\[test\]$/gm) || []).length;
}

const names = Object.keys(ex).filter((k) => k.startsWith('check_') && k !== 'check_count');
let ran = 0;
let failed = 0;
for (const name of names) {
  const t = performance.now();
  try {
    if (ex[name]() !== 1) throw new Error('returned ' + ex[name]());
    ran += 1;
    console.log(`ok   ${name} (${(performance.now() - t).toFixed(0)} ms)`);
  } catch (e) {
    failed += 1;
    console.log(`FAIL ${name}: ${e.message}`);
  }
}
const count = ex.check_count();
console.log(`ran ${ran} of ${count} checks (${tests} #[test] in the included test files)`);

const n = 2000;
const t = performance.now();
ex.bench_sky(n);
const us = ((performance.now() - t) * 1000) / n;
const bytes = fs.statSync(wasmPath).size;
const hex = (p) => Buffer.from(new Uint8Array(ex.memory.buffer, p, 32)).toString('hex');
const laws = ex.law_count();
for (let i = 0; i < laws; i += 1) console.log(`report: law ${i} semantics ${hex(ex.law_semantics_id(i))}`);
if (laws === 0 || ex.law_semantics_id(laws) !== 0) {
  console.error('browser lane: law_count() must be non-zero and law_semantics_id(law_count()) null');
  process.exit(1);
}
console.log(`report: horizon::sky ${us.toFixed(1)} µs/call (node, ${n} calls); browser build ${bytes} bytes`);

if (failed > 0 || ran === 0 || ran !== count || count !== tests) {
  console.error('browser lane: every check must run and pass, one per #[test]');
  process.exit(1);
}
