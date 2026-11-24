import { expect, test } from 'bun:test';
import { parseScript, splitArguments } from './mi-kubejs';

// An invented script of the same shape as the declarative calls the reader knows; its blocks and keys are made up.
const FIXTURE = `
let toy
MIMachineEvents.registerRecipeTypes(api => {
    toy = api.register('toy_press')
        .withItemInputs()
        .withFluidOutputs()
})
MIMachineEvents.registerMachines(api => {
    // a comment
    const wall = api.memberOfBlock('test:wall')
    const lamp = api.memberOfBlock('test:lamp')
    const port = api.hatchOf('item_input', 'energy_input')
    const shape = api.layeredShape('toy_casing', [
        ['wpw', 'w#w', 'www'],
        ['wlw', '   ', 'www']
    ])
        .key('w', wall, api.noHatch())
        .key('l', lamp, api.noHatch())
        .key('p', wall, port)
        .build()
    api.simpleElectricCraftingMultiBlock(
        'Toy Press', 'toy_press', toy, shape,
        api.progressBar(1, 2, 'arrow'),
        itemInputs => itemInputs.addSlots(3, 4, 2, 1),
        itemOutputs => itemOutputs.addSlot(9, 9),
        fluidInputs => {},
        fluidOutputs => fluidOutputs.addSlot(5, 6),
        'toy_casing', 'toys', true, false, false
    )
})
`;

test('splitArguments keeps strings and brackets whole', () => {
  expect(splitArguments("'a,b', f(1, 2), [3, 4], x")).toEqual(["'a,b'", 'f(1, 2)', '[3, 4]', 'x']);
});

test('a toy script becomes one multiblock', () => {
  const [m, ...rest] = parseScript(FIXTURE);
  expect(rest).toEqual([]);
  expect(m.id).toBe('toy_press');
  expect(m.name).toBe('Toy Press');
  expect(m.recipe_type).toBe('toy_press');
  expect(m.rules).toEqual({ item_inputs: true, item_outputs: false, fluid_inputs: false, fluid_outputs: true });
  expect(m.hatch_casing).toBe('toy_casing');
  expect(m.layers).toEqual([['wpw', 'w#w', 'www'], ['wlw', '   ', 'www']]);
  expect(m.keys.w).toEqual({ block: 'test:wall', hatches: null });
  expect(m.keys.p).toEqual({ block: 'test:wall', hatches: ['item_input', 'energy_input'] });
  expect(m.slots).toEqual({ item_inputs: { x: 3, y: 4, cols: 2, rows: 1 }, item_outputs: { x: 9, y: 9, cols: 1, rows: 1 }, fluid_outputs: { x: 5, y: 6, cols: 1, rows: 1 } });
});

test('a script with no machine reads as nothing', () => {
  expect(parseScript("MIMachineEvents.registerCasings(api => { api.registerBlockImitation('c', 'test:wall') })")).toEqual([]);
});

test('unknown forms throw rather than guess', () => {
  expect(() => parseScript(FIXTURE.replace('.withFluidOutputs()', '.withMagic()'))).toThrow('withMagic');
  expect(() => parseScript(FIXTURE.replace("api.hatchOf('item_input', 'energy_input')", 'api.somethingElse()'))).toThrow('hatch form');
  expect(() => parseScript(FIXTURE.replace('.build()', ''))).toThrow('.build()');
  expect(() => parseScript(FIXTURE.replace('fluidInputs => {}', 'fluidInputs => compute()'))).toThrow('slot lambda');
});
