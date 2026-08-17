/*
 * Smoke test for the emscripten build: `node test/wasm_smoke.js` after `make emscripten`.
 *
 * It exists because a wasm module that exports nothing still loads happily -- without an
 * explicit `-sEXPORTED_FUNCTIONS`, `-O2` strips the whole library, and the failure is
 * silent until something tries to call in.  The expected coordinates are the ones the
 * native `flatapi` test produces, so the two builds are checked against each other.
 */

const path = require('path');
const Module = require(path.join(__dirname, '..', 'src', 'non-layered-tidy-trees.js'));

const expected = [
  { x: 5, y: 5 },
  { x: 0, y: 20 },
  { x: 10, y: 20 },
];

Module.onRuntimeInitialized = () => {
  const n = 3;
  const wh = [10, 10, 10, /* h */ 10, 10, 10, /* margin */ 0, 0, 0];
  const whg = [10, 10, 10, /* h */ 5, 5, 5];
  const children = [2, 0, 0, /* adjacency */ 2, 3];

  const pwh = Module._malloc(wh.length * 8);
  const pwhg = Module._malloc(whg.length * 8);
  const pch = Module._malloc(children.length * 4);
  Module.HEAPF64.set(wh, pwh / 8);
  Module.HEAPF64.set(whg, pwhg / 8);
  Module.HEAP32.set(children, pch / 4);

  // n, wh, whg, children, rooti, vertically, centeredxy, x, y
  Module.ccall('layout_api', null,
    ['number', 'number', 'number', 'number', 'number', 'number', 'number', 'number', 'number'],
    [n, pwh, pwhg, pch, 1, 1, 0, 0, 0]);

  const out = Module.HEAPF64.subarray(pwh / 8, pwh / 8 + 2 * n);

  let failures = 0;
  for (let i = 0; i < n; i++) {
    const x = out[i], y = out[i + n];
    const ok = x === expected[i].x && y === expected[i].y;
    if (!ok) failures++;
    console.log(`  ${ok ? 'PASS' : 'FAIL'}  node ${i + 1}: x=${x} y=${y}` +
      (ok ? '' : ` (expected x=${expected[i].x} y=${expected[i].y})`));
  }

  Module._free(pwh); Module._free(pwhg); Module._free(pch);
  process.exitCode = failures === 0 ? 0 : 1;
};
