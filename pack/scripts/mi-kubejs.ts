// Reads the multiblock scripts of the player's ATM10 instance (kubejs/startup_scripts/Modern-Industrialization/
// multiblocks/*.js) at stage time and writes what they declare as JSON into the staged copy of the bundle
// (mi-core/data/kubejs_multiblocks.json), where mi-core's build embeds it. Nothing of the scripts is kept in this
// repository: the shapes, their blocks and keys exist only in the staged, untracked copy. A script that uses a form
// this reader does not know is skipped with one line naming it.
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

export interface KubeMultiblock {
  id: string;
  name: string;
  recipe_type: string;
  rules: { item_inputs: boolean; item_outputs: boolean; fluid_inputs: boolean; fluid_outputs: boolean };
  hatch_casing: string;
  layers: string[][];
  keys: Record<string, { block: string; hatches: string[] | null }>;
  slots: Record<string, { x: number; y: number; cols: number; rows: number }>;
}

export const SCRIPTS = 'kubejs/startup_scripts/Modern-Industrialization/multiblocks';
export const DATA_FILE = 'mi-core/data/kubejs_multiblocks.json';

class Unknown extends Error {}

/** Splits at the commas outside of strings and brackets. */
export function splitArguments(text: string): string[] {
  const parts: string[] = [];
  let depth = 0, quote = '', current = '';
  for (const c of text) {
    if (quote) { current += c; if (c === quote) quote = ''; continue; }
    if (c === "'" || c === '"') { quote = c; current += c; continue; }
    if ('([{'.includes(c)) depth++;
    if (')]}'.includes(c)) depth--;
    if (c === ',' && depth === 0) { parts.push(current.trim()); current = ''; continue; }
    current += c;
  }
  if (current.trim()) parts.push(current.trim());
  return parts;
}

/** The text between the bracket at `open` and its match, and the index after the match. */
function balanced(text: string, open: number): [string, number] {
  const pairs: Record<string, string> = { '(': ')', '[': ']', '{': '}' };
  const stack: string[] = [];
  let quote = '';
  for (let i = open; i < text.length; i++) {
    const c = text[i];
    if (quote) { if (c === quote) quote = ''; continue; }
    if (c === "'" || c === '"') { quote = c; continue; }
    if (pairs[c]) stack.push(pairs[c]);
    else if (c === stack[stack.length - 1]) {
      stack.pop();
      if (stack.length === 0) return [text.slice(open + 1, i), i + 1];
    }
  }
  throw new Unknown('unbalanced brackets');
}

const stringOf = (arg: string | undefined): string => {
  const m = /^'([^']*)'$|^"([^"]*)"$/.exec(arg ?? '');
  if (!m) throw new Unknown(`not a string literal: ${arg}`);
  return m[1] ?? m[2];
};

const RULE_CALLS: Record<string, keyof KubeMultiblock['rules']> = {
  withItemInputs: 'item_inputs', withItemOutputs: 'item_outputs', withFluidInputs: 'fluid_inputs', withFluidOutputs: 'fluid_outputs',
};

/** `name = x.register('id').withItemInputs()...`: the recipe types of one script, by the variable they are kept in. */
function recipeTypes(text: string): Map<string, { id: string; rules: KubeMultiblock['rules'] }> {
  const types = new Map<string, { id: string; rules: KubeMultiblock['rules'] }>();
  const re = /(\w+)\s*=\s*\w+\s*\.register\(\s*('[^']*'|"[^"]*")\s*\)((?:\s*\.\w+\(\s*\))*)/g;
  for (const m of text.matchAll(re)) {
    const rules = { item_inputs: false, item_outputs: false, fluid_inputs: false, fluid_outputs: false };
    for (const call of m[3].matchAll(/\.(\w+)\(/g)) {
      const rule = RULE_CALLS[call[1]];
      if (!rule) throw new Unknown(`recipe type call ${call[1]}()`);
      rules[rule] = true;
    }
    types.set(m[1], { id: stringOf(m[2]), rules });
  }
  return types;
}

type Shape = Pick<KubeMultiblock, 'hatch_casing' | 'layers' | 'keys'>;

function layeredShapes(text: string, members: Map<string, string>, hatches: Map<string, string[]>): Map<string, Shape> {
  const shapes = new Map<string, Shape>();
  for (const m of text.matchAll(/(?:const|let)\s+(\w+)\s*=\s*\w+\s*\.layeredShape\(/g)) {
    const [body, after] = balanced(text, m.index! + m[0].length - 1);
    const [casing, layerList] = [splitArguments(body)[0], body.slice(body.indexOf(',') + 1)];
    const layers: string[][] = [];
    const outer = layerList.trim();
    if (!outer.startsWith('[')) throw new Unknown('layers are not a literal list');
    const [inner] = balanced(outer, 0);
    for (let i = 0; i < inner.length; i++) {
      if (inner[i] !== '[') continue;
      const [layer, end] = balanced(inner, i);
      layers.push([...layer.matchAll(/'([^']*)'|"([^"]*)"/g)].map(s => s[1] ?? s[2]));
      i = end - 1;
    }
    const keys: Shape['keys'] = {};
    let rest = text.slice(after);
    for (;;) {
      const k = /^\s*\.key\(/.exec(rest);
      if (!k) break;
      const [call, end] = balanced(rest, k[0].length - 1);
      const [chr, member, hatch] = splitArguments(call);
      const key = stringOf(chr);
      if (key.length !== 1) throw new Unknown(`key ${chr}`);
      const block = members.get(member);
      if (!block) throw new Unknown(`key ${key} uses ${member}, which is no memberOfBlock`);
      let flags: string[] | null;
      if (/^\w+\.noHatch\(\s*\)$/.test(hatch ?? '')) flags = null;
      else if (hatch && hatches.has(hatch)) flags = hatches.get(hatch)!;
      else throw new Unknown(`key ${key} uses the hatch form ${hatch}`);
      keys[key] = { block, hatches: flags };
      rest = rest.slice(end);
    }
    if (!/^\s*\.build\(\s*\)/.test(rest)) throw new Unknown(`shape ${m[1]} does not end in .build()`);
    if (layers.length === 0) throw new Unknown(`shape ${m[1]} has no layers`);
    shapes.set(m[1], { hatch_casing: stringOf(casing), layers, keys });
  }
  return shapes;
}

/** A slot lambda: `x => x.addSlots(3, 4, 2, 1)`, `x => x.addSlot(9, 9)` or an empty body. */
function slotLambda(arg: string): { x: number; y: number; cols: number; rows: number } | null {
  if (/^\w+\s*=>\s*\{\s*\}$/.test(arg)) return null;
  const many = /^\w+\s*=>\s*\w+\.addSlots\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)$/.exec(arg);
  if (many) return { x: +many[1], y: +many[2], cols: +many[3], rows: +many[4] };
  const one = /^\w+\s*=>\s*\w+\.addSlot\(\s*(\d+)\s*,\s*(\d+)\s*\)$/.exec(arg);
  if (one) return { x: +one[1], y: +one[2], cols: 1, rows: 1 };
  throw new Unknown(`slot lambda ${arg}`);
}

/** Every multiblock one script registers. Throws Unknown for a form it does not read. */
export function parseScript(source: string): KubeMultiblock[] {
  const text = source.replace(/^\s*\/\/.*$/gm, '');
  const members = new Map<string, string>();
  for (const m of text.matchAll(/(?:const|let)\s+(\w+)\s*=\s*\w+\s*\.memberOfBlock\(\s*('[^']*'|"[^"]*")\s*\)/g)) members.set(m[1], stringOf(m[2]));
  const hatches = new Map<string, string[]>();
  for (const m of text.matchAll(/(?:const|let)\s+(\w+)\s*=\s*\w+\s*\.hatchOf\(([^)]*)\)/g)) hatches.set(m[1], splitArguments(m[2]).map(stringOf));
  const types = recipeTypes(text);
  const shapes = layeredShapes(text, members, hatches);
  const out: KubeMultiblock[] = [];
  for (const m of text.matchAll(/\w+\s*\.simpleElectricCraftingMultiBlock\(/g)) {
    const [body] = balanced(text, m.index! + m[0].length - 1);
    const a = splitArguments(body);
    if (a.length < 10) throw new Unknown('simpleElectricCraftingMultiBlock with fewer than 10 arguments');
    const type = types.get(a[2]);
    if (!type) throw new Unknown(`recipe type ${a[2]} is not registered with register()`);
    const shape = shapes.get(a[3]);
    if (!shape) throw new Unknown(`shape ${a[3]} is not a layeredShape`);
    const slots: KubeMultiblock['slots'] = {};
    for (const [name, arg] of [['item_inputs', a[5]], ['item_outputs', a[6]], ['fluid_inputs', a[7]], ['fluid_outputs', a[8]]] as const) {
      const slot = slotLambda(arg);
      if (slot) slots[name] = slot;
    }
    out.push({ id: stringOf(a[1]), name: stringOf(a[0]), recipe_type: type.id, rules: type.rules, ...shape, hatch_casing: stringOf(a[9]), slots });
  }
  return out;
}

/** Reads every script of `instance`; returns the multiblocks and one line for each script left out. */
export function readInstance(instance: string): { multiblocks: KubeMultiblock[]; skipped: string[] } {
  const dir = join(instance, SCRIPTS);
  const multiblocks: KubeMultiblock[] = [], skipped: string[] = [];
  if (!existsSync(dir)) return { multiblocks, skipped: [`no ${dir}`] };
  for (const file of readdirSync(dir).filter(f => f.endsWith('.js')).sort()) {
    const source = readFileSync(join(dir, file), 'utf8');
    try {
      multiblocks.push(...parseScript(source));
    } catch (error) {
      if (!(error instanceof Unknown)) throw error;
      skipped.push(`${file}: ${error.message}`);
    }
  }
  return { multiblocks, skipped };
}

/** Writes `multiblocks` to `<bundle>/mi-core/data/kubejs_multiblocks.json`. */
export function writeData(bundle: string, multiblocks: KubeMultiblock[]): string {
  const file = join(bundle, DATA_FILE);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, JSON.stringify({ multiblocks }, null, 1) + '\n');
  return file;
}
