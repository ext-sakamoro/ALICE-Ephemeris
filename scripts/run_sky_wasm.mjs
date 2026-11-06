// node scripts/run_sky_wasm.mjs <sky.wasm> <expected.txt>
// The page's module, checked as a page uses it: it exports only `sun_alt_az`, `moon_alt_az_lit` and
// `memory`; over the
// inputs in expected.txt it gives the outputs whose sha256 the native test pinned there (0 inputs
// fail); at the 2026 March equinox the Sun is overhead at the subsolar point and below the
// antipode's horizon; what is not a time or a place gives NaN.
import fs from 'node:fs';
import crypto from 'node:crypto';

const [wasmPath, expectedPath] = process.argv.slice(2);
const bytes = fs.readFileSync(wasmPath);
const { instance } = await WebAssembly.instantiate(bytes, {});
const ex = instance.exports;
const problems = [];

const names = Object.keys(ex).sort();
if (names.join(',') !== 'memory,moon_alt_az_lit,stars_alt_az_mag,stars_len,stars_table,stars_table_written,sun_alt_az') problems.push(`exports: ${names.join(', ')}`);

const sun = (ms, lat, lon) => Array.from(new Float64Array(ex.memory.buffer, ex.sun_alt_az(ms, lat, lon), 2));

const moon = (ms, lat, lon) => Array.from(new Float64Array(ex.memory.buffer, ex.moon_alt_az_lit(ms, lat, lon), 3));

let pinned = null;
let moonPinned = null;
let n = 0;
const h = crypto.createHash('sha256');
const hm = crypto.createHash('sha256');
const hs = crypto.createHash('sha256');
const table = fs.readFileSync(new URL('../sky/stars.bin', import.meta.url));
if (!Number.isNaN(new Float64Array(ex.memory.buffer, ex.stars_alt_az_mag(0, 0, 0), 1)[0])) problems.push('stars gave a number with no table');
if (ex.stars_table_written(table.length - 1) !== 0) problems.push('a short table was taken');
new Uint8Array(ex.memory.buffer, ex.stars_table(), table.length).set(table);
if (ex.stars_table_written(table.length) !== 1) problems.push('the whole table was refused');
const stars = (ms, lat, lon) => Array.from(new Float64Array(ex.memory.buffer, ex.stars_alt_az_mag(ms, lat, lon), ex.stars_len() * 3));
let starsPinned = null;
for (const line of fs.readFileSync(expectedPath, 'utf8').split('\n')) {
  if (line.startsWith('#') || line.trim() === '') continue;
  if (line.startsWith('sha256 ')) { pinned = line.slice(7).trim(); continue; }
  if (line.startsWith('moon-sha256 ')) { moonPinned = line.slice(12).trim(); continue; }
  if (line.startsWith('stars-sha256 ')) { starsPinned = line.slice(13).trim(); continue; }
  const [ms, lat, lon] = line.trim().split(/\s+/).map(Number);
  const out = new Float64Array(sun(ms, lat, lon));
  h.update(Buffer.from(out.buffer));
  hm.update(Buffer.from(new Float64Array(moon(ms, lat, lon)).buffer));
  hs.update(Buffer.from(new Float64Array(stars(ms, lat, lon)).buffer));
  n += 1;
}
const got = h.digest('hex');
if (n === 0) problems.push('no inputs compared');
if (got !== pinned) problems.push(`outputs ${got} != pinned ${pinned}`);
const gotStars = hs.digest('hex');
if (gotStars !== starsPinned) problems.push(`stars outputs ${gotStars} != pinned ${starsPinned}`);
const gotMoon = hm.digest('hex');
if (gotMoon !== moonPinned) problems.push(`moon outputs ${gotMoon} != pinned ${moonPinned}`);
console.log(`${got === pinned && gotMoon === moonPinned && n > 0 ? 'ok  ' : 'FAIL'} native bits over ${n} inputs, Sun and Moon`);

const deg = 180 / Math.PI;
const equinox = Date.UTC(2026, 2, 20, 14, 46);
const subsolar = -(15 * (14 + 46 / 60) - 180 - 7.5 / 4);
const [up] = sun(equinox, 0, subsolar);
const [down] = sun(equinox, 0, subsolar + 180);
const nan = [sun(equinox + 0.5, 0, 0), sun(Date.UTC(1900, 0, 1), 0, 0), sun(NaN, 0, 0), sun(equinox, NaN, 0), sun(equinox, 91, 0), sun(equinox, 0, 181)];
const okSun = up * deg > 89 && down * deg < -89 && nan.every(([a, z]) => Number.isNaN(a) && Number.isNaN(z));
if (!okSun) problems.push(`equinox ${up * deg}°/${down * deg}° or a NaN case`);
console.log(`${okSun ? 'ok  ' : 'FAIL'} sun_alt_az: subsolar ${(up * deg).toFixed(3)}°, antipode ${(down * deg).toFixed(3)}°`);
console.log(`report: sky module ${bytes.length} bytes`);

if (problems.length > 0) {
  console.error(`sky module: ${problems.join('; ')}`);
  process.exit(1);
}
