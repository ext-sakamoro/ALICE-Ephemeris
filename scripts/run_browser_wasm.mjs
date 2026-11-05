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
console.log(`report: det-math semantics ${hex(ex.det_math_semantics_id())}`);
console.log(`report: horizon::sky ${us.toFixed(1)} µs/call (node, ${n} calls); browser build ${bytes} bytes`);

// The export a page calls, called as a page calls it (Date.now(), degrees): at the March equinox
// of 2026 (2026-03-20 14:46 UTC) the Sun is overhead at the subsolar point (latitude 0,
// longitude −(15°/h · UT − 180° + the equation of time, −7.5 min)) and below the antipode's
// horizon; a time that is not an integral millisecond, or before UTC, gives NaN
const sun = (ms, lat, lon) => {
  const p = ex.sun_alt_az(ms, lat, lon);
  return Array.from(new Float64Array(ex.memory.buffer, p, 2));
};
const equinox = Date.UTC(2026, 2, 20, 14, 46);
const subsolar = -(15 * (14 + 46 / 60) - 180 - 7.5 / 4);
const deg = 180 / Math.PI;
const [up] = sun(equinox, 0, subsolar);
const [down] = sun(equinox, 0, subsolar + 180);
const nan = [sun(equinox + 0.5, 0, 0), sun(Date.UTC(1900, 0, 1), 0, 0), sun(NaN, 0, 0), sun(equinox, NaN, 0)];
const exportOk = up * deg > 89 && down * deg < -89 && nan.every(([a, z]) => Number.isNaN(a) && Number.isNaN(z));
console.log(`${exportOk ? 'ok  ' : 'FAIL'} sun_alt_az: subsolar ${(up * deg).toFixed(3)}°, antipode ${(down * deg).toFixed(3)}°, invalid inputs NaN ${nan.every(([a]) => Number.isNaN(a))}`);
if (!exportOk) failed += 1;

if (failed > 0 || ran === 0 || ran !== count || count !== tests) {
  console.error('browser lane: every check must run and pass, one per #[test]');
  process.exit(1);
}
