use super::*;

fn item(id: &str, name: &str, tags: &[&str]) -> Item {
    Item { id: id.into(), name: name.into(), tags: tags.iter().map(|t| t.to_string()).collect(), ..Item::default() }
}

fn list() -> List {
    List::new(vec![
        item("mekanism:osmium_ingot", "Osmium Ingot", &["c:ingots", "c:ingots/osmium"]),
        item("ae2:fluix_crystal", "Fluix Crystal", &["ae2:all_fluix"]),
        item("minecraft:oak_log", "Oak Log", &["minecraft:logs"]),
        item("minecraft:iron_ingot", "Iron Ingot", &["c:ingots", "c:ingots/iron"]),
        item("minecraft:red_wool", "Red Wool", &["minecraft:wool"]),
        Item { tooltip: vec!["Stores energy".into()], colors: vec!["red".into()], creative_tabs: vec!["Mekanism".into()], ..item("mekanism:basic_energy_cube", "Basic Energy Cube", &[]) },
    ])
}

fn ids(l: &List, q: &str, modes: &Modes) -> Vec<String> {
    l.filter(q, modes).into_iter().map(|n| l.id(n).to_owned()).collect()
}

#[test]
fn order_is_minecraft_then_mods_by_name() {
    let l = list();
    let all: Vec<&str> = (0..l.len()).map(|n| l.id(n)).collect();
    assert_eq!(all, ["minecraft:oak_log", "minecraft:iron_ingot", "minecraft:red_wool", "ae2:fluix_crystal", "mekanism:osmium_ingot", "mekanism:basic_energy_cube"]);
}

#[test]
fn words_and_alternatives() {
    let (l, m) = (list(), Modes::default());
    assert_eq!(ids(&l, "", &m).len(), 6);
    assert_eq!(ids(&l, "INGOT", &m), ["minecraft:iron_ingot", "mekanism:osmium_ingot"]);
    assert_eq!(ids(&l, "iron ingot", &m), ["minecraft:iron_ingot"]);
    assert_eq!(ids(&l, "oak | fluix", &m), ["minecraft:oak_log", "ae2:fluix_crystal"]);
    assert_eq!(ids(&l, "ingot -iron", &m), ["mekanism:osmium_ingot"]);
    assert_eq!(ids(&l, "\"oak log\"", &m), ["minecraft:oak_log"]);
    assert_eq!(ids(&l, "oak\\ log", &m), ["minecraft:oak_log"]);
    assert!(ids(&l, "\"log oak\"", &m).is_empty());
}

#[test]
fn prefixes_with_jei_defaults() {
    let (l, m) = (list(), Modes::default());
    assert_eq!(ids(&l, "@mekanism", &m), ["mekanism:osmium_ingot", "mekanism:basic_energy_cube"]);
    assert_eq!(ids(&l, "@applied", &m), ["ae2:fluix_crystal"]);
    assert_eq!(ids(&l, "@appliedenergistics", &m), ["ae2:fluix_crystal"]);
    assert_eq!(ids(&l, "-@minecraft", &m).len(), 3);
    assert_eq!(ids(&l, "#c:ingots", &m), ["minecraft:iron_ingot", "mekanism:osmium_ingot"]);
    assert_eq!(ids(&l, "#logs", &m), ["minecraft:oak_log"]);
    assert_eq!(ids(&l, "@minecraft ingot", &m), ["minecraft:iron_ingot"]);
    // Mod names and tags need their prefix by default; the tooltip is searched without one.
    assert!(ids(&l, "mekanism", &m).is_empty());
    assert_eq!(ids(&l, "stores", &m), ["mekanism:basic_energy_cube"]);
    assert_eq!(ids(&l, "$energy", &m), ["mekanism:basic_energy_cube"]);
    // Colour, identifier and creative tab are off by default: the prefix is plain text and matches no name.
    assert!(ids(&l, "^red", &m).is_empty());
    assert!(ids(&l, "&oak", &m).is_empty());
    assert!(ids(&l, "%mekanism", &m).is_empty());
}

#[test]
fn modes_change_what_a_prefix_does() {
    let l = list();
    let mut m = Modes::default();
    m.set(Prefix::Color, Mode::RequirePrefix);
    m.set(Prefix::Identifier, Mode::RequirePrefix);
    m.set(Prefix::CreativeTab, Mode::RequirePrefix);
    assert_eq!(ids(&l, "^red", &m), ["mekanism:basic_energy_cube"]);
    assert_eq!(ids(&l, "&minecraft:oak", &m), ["minecraft:oak_log"]);
    assert_eq!(ids(&l, "%mekanism", &m), ["mekanism:basic_energy_cube"]);
    m.set(Prefix::ModName, Mode::Enabled);
    assert_eq!(ids(&l, "mekanism", &m), ["mekanism:osmium_ingot", "mekanism:basic_energy_cube"]);
    m.set(Prefix::Tooltip, Mode::Disabled);
    assert!(ids(&l, "stores", &m).is_empty());
    assert!(ids(&l, "$energy", &m).is_empty());
}

#[test]
fn modes_round_trip_and_cycle() {
    for mode in [Mode::Enabled, Mode::RequirePrefix, Mode::Disabled] {
        assert_eq!(Mode::parse(mode.name()), Some(mode));
    }
    assert_eq!(Mode::Disabled.next(), Mode::Enabled);
    assert_eq!(Modes::default().get(Prefix::Tooltip), Mode::Enabled);
    assert_eq!(give_count(false, false), 1);
    assert_eq!(give_count(true, false), 64);
}

mod recipe_screen {
    use crate::recipes::*;

    fn s(item: &str) -> Stack {
        Stack::of(item)
    }
    fn book() -> Book {
        let crafting = Category {
            id: "minecraft:crafting".into(),
            title: "Crafting".into(),
            icon: s("minecraft:crafting_table"),
            catalysts: vec![s("minecraft:crafting_table"), s("minecraft:crafter")],
            recipes: vec![
                Recipe::shaped(3, 3, vec![vec![s("minecraft:iron_nugget")]; 9], vec![s("minecraft:iron_ingot")]),
                Recipe::shapeless(vec![vec![s("minecraft:iron_ingot")]], vec![Stack { count: 9, ..s("minecraft:iron_nugget") }]),
            ],
        };
        let smelting = Category {
            id: "minecraft:smelting".into(),
            title: "Smelting".into(),
            icon: s("minecraft:furnace"),
            catalysts: vec![s("minecraft:furnace")],
            recipes: vec![Recipe {
                shape: Shape::Cooking,
                inputs: vec![vec![s("minecraft:raw_iron"), s("minecraft:iron_ore")]],
                outputs: vec![vec![Output::always(s("minecraft:iron_ingot"))]],
                notes: vec![cooking_note(0.7, 200)],
                ..Recipe::default()
            }],
        };
        let crusher = Category {
            id: "mekanism:crusher".into(),
            title: "Crusher".into(),
            icon: s("mekanism:crusher"),
            catalysts: vec![s("mekanism:crusher")],
            recipes: vec![Recipe::machine(
                vec![vec![s("minecraft:iron_ingot")]],
                vec![
                    vec![Output::always(s("mekanism:dust_iron"))],
                    vec![Output { stack: s("mekanism:dust_tin"), chance: 0.25, min: 1, max: 2, notes: vec![] }],
                ],
                vec!["Energy: 50 J/t".into(), "Recipe: mekanism:crushing/iron_ingot_to_dust".into()],
            )],
        };
        let names = [("mekanism:dust_tin", "Tin Dust")].map(|(a, b)| (a.to_string(), b.to_string()));
        Book { categories: vec![crafting, smelting, crusher], names: names.into_iter().collect() }
    }
    fn focus(item: &str, mode: Mode) -> Focus {
        Focus { item: item.into(), mode }
    }

    #[test]
    fn jei_places_small_recipes_in_the_three_by_three_grid() {
        assert_eq!((0..3).map(|i| crafting_index(i, 1, 3)).collect::<Vec<_>>(), [1, 4, 7]);
        assert_eq!(crafting_index(0, 1, 1), 4);
        assert_eq!((0..4).map(|i| crafting_index(i, 2, 2)).collect::<Vec<_>>(), [0, 1, 3, 4]);
        assert_eq!((0..3).map(|i| crafting_index(i, 3, 1)).collect::<Vec<_>>(), [3, 4, 5]);
        let r = Recipe::shapeless(vec![vec![s("a")], vec![s("b")], vec![s("c")]], vec![s("d")]);
        assert_eq!(r.inputs.iter().map(|c| c.len()).collect::<Vec<_>>(), [1, 1, 0, 1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn r_shows_the_categories_that_make_it_and_u_the_ones_that_take_it() {
        let mut v = Viewer::new(book());
        assert!(v.open(focus("minecraft:iron_ingot", Mode::Recipes)));
        let p = v.page().unwrap();
        assert_eq!(p.title, "Crafting");
        assert_eq!(p.tabs.iter().map(|t| t.item.as_str()).collect::<Vec<_>>(), ["minecraft:crafting_table", "minecraft:furnace"]);
        assert_eq!(p.catalysts.len(), 2);
        assert_eq!(p.views.len(), 1);
        v.tab(1);
        let p = v.page().unwrap();
        assert_eq!((p.title.as_str(), p.views[0].note.as_str()), ("Smelting", "0.7 XP, 10s"));
        // U: the crafting and the crusher take it; the furnace's own U shows all of smelting.
        assert!(v.open(focus("minecraft:iron_ingot", Mode::Uses)));
        let titles = |v: &Viewer| v.current().unwrap().found.iter().map(|f| v.book.categories[f.category].id.clone()).collect::<Vec<_>>();
        assert_eq!(titles(&v), ["minecraft:crafting", "mekanism:crusher"]);
        assert!(v.open(focus("minecraft:furnace", Mode::Uses)));
        assert_eq!(titles(&v), ["minecraft:smelting"]);
        assert!(!v.open(focus("minecraft:dirt", Mode::Recipes)), "nothing makes dirt: nothing opens");
        assert_eq!(titles(&v), ["minecraft:smelting"]);
    }

    #[test]
    fn backspace_walks_the_history_back_to_the_list() {
        let mut v = Viewer::new(book());
        v.open(focus("minecraft:iron_ingot", Mode::Recipes));
        v.tab(1);
        v.open(focus("minecraft:raw_iron", Mode::Uses));
        assert_eq!(v.history(), 1);
        assert!(v.back());
        let s = v.current().unwrap();
        assert_eq!((s.focus.item.as_str(), s.tab), ("minecraft:iron_ingot", 1), "back to the tab it was on");
        assert!(!v.back(), "then the screen closes");
        assert!(v.page().is_none());
    }

    #[test]
    fn a_machine_shows_its_chance_outputs_after_the_result() {
        let mut v = Viewer::new(book());
        v.open(focus("minecraft:iron_ingot", Mode::Uses));
        v.tab(1);
        let p = v.page().unwrap();
        assert_eq!(p.title, "Crusher");
        let view = &p.views[0];
        assert_eq!(view.shape, Shape::Single);
        assert_eq!(view.slots.iter().map(|s| s[0].item.as_str()).collect::<Vec<_>>(), ["minecraft:iron_ingot", "mekanism:dust_tin"]);
        assert_eq!(view.results[0].item, "mekanism:dust_iron");
        assert_eq!(view.note, "Tin Dust: 25%\nTin Dust: 1-2\nEnergy: 50 J/t");
        // Smelting's U on raw iron locks the cycling input on it.
        v.open(focus("minecraft:raw_iron", Mode::Uses));
        assert_eq!(v.page().unwrap().views[0].slots[0], vec![s("minecraft:raw_iron")]);
        assert_eq!(percent(0.005), "0.5");
        assert_eq!(wrap("a bb ccc dddd", 6), ["a bb", "ccc", "dddd"]);
    }
}

mod window {
    use crate::recipes::Shape;
    use crate::window::*;

    #[test]
    fn the_window_is_centred_with_jeis_sizes_and_keeps_clear_of_the_list() {
        // 960x540 at GUI scale 2, the item list from x 348 (creative): JEI's 198 px window, centred.
        let s = Sizes { screen: [480.0, 270.0], panel_left: Some(348.0), tabs: 3, tab: 1, catalysts: 2, widest: 116.0 };
        let w = window(&s);
        assert_eq!(w.area, [141.0, 39.0, 198.0, 212.0]);
        assert_eq!(w.tabs.len(), 3);
        assert_eq!(w.tabs[0], [143.0, 18.0, 24.0, 24.0], "the tabs overlap the window's top by 3 px");
        assert_eq!(w.catalyst_area, Some([119.0, 39.0, 28.0, 46.0]), "the catalysts' tab overlaps the left edge by 6 px");
        assert_eq!(w.catalysts[1], [124.0, 62.0, 18.0, 18.0]);
        assert_eq!(w.category_arrows[0], [147.0, 45.0, 13.0, 13.0]);
        assert_eq!(w.page_arrows[1], [320.0, 60.0, 13.0, 13.0]);
        assert_eq!(w.recipes[1], 77.0);
        // The survival inventory's list starts at 330: the window moves left of it.
        let w = window(&Sizes { panel_left: Some(330.0), ..s });
        assert_eq!(w.area[0] + w.area[2], 328.0);
        // A recipe wider than the window widens it.
        assert_eq!(window(&Sizes { widest: 220.0, panel_left: None, ..s }).area[2], 232.0);
    }

    #[test]
    fn tabs_page_and_catalysts_wrap_into_columns() {
        let s = Sizes { screen: [480.0, 270.0], panel_left: None, tabs: 12, tab: 9, catalysts: 14, widest: 0.0 };
        let w = window(&s);
        // Eight 24 px tabs fit in 198 px; the selected ninth is on the second page of them.
        assert_eq!((w.first_tab, w.tabs.len()), (8, 4));
        // 11 rows fit in the 212 px tall window: 14 catalysts take two columns, the first beside the window.
        let area = w.catalyst_area.unwrap();
        assert_eq!(area[2], 2.0 * SLOT + 10.0);
        assert!(w.catalysts[0][0] > w.catalysts[13][0]);
        assert!(area[0] >= 0.0 && w.area[0] - area[0] == area[2] - 6.0);
    }

    #[test]
    fn recipes_lay_out_as_jeis_vanilla_categories() {
        // Crafting: the 3x3 grid, the arrow, the result beside it.
        let p = places(Shape::Grid, 3, 3, 9);
        assert_eq!(p.slots.len(), 10);
        assert_eq!((p.slots[4], p.slots[9], p.arrow), ([18.0, 18.0], [94.0, 18.0], [61.0, 19.0]));
        assert_eq!(p.size, [116.0, 54.0], "JEI's 116 x 54, the output slot's rim included");
        // Smelting: the input over the flame, 82 px wide.
        let p = places(Shape::Cooking, 1, 1, 1);
        assert_eq!((p.slots[0], p.slots[1], p.flame, p.arrow), ([0.0, 0.0], [60.0, 18.0], Some([1.0, 20.0]), [26.0, 17.0]));
        assert_eq!(p.size, [82.0, 54.0]);
        // A machine with one input and six outputs: the extra five after the result, four to a row.
        let p = places(Shape::Single, 1, 1, 6);
        assert_eq!(p.slots.len(), 7);
        assert_eq!(p.slots[6], [54.0, 0.0], "the result last");
        assert_eq!((p.slots[1], p.slots[4], p.slots[5]), ([76.0, 0.0], [130.0, 0.0], [76.0, 18.0]));
        assert_eq!(p.size, [148.0, 36.0]);
    }

    #[test]
    fn pages_hold_the_recipes_that_fit_and_spread_them() {
        assert_eq!(pages(&[54.0; 5], 135.0), [0, 2, 4]);
        assert_eq!(pages(&[200.0, 20.0], 135.0), [0, 1], "a tall recipe has a page of its own");
        assert_eq!(spread(&[54.0, 54.0], 100.0, 135.0), [106.0, 173.0]);
        assert!(inside([10.0, 10.0], [0.0, 0.0, 18.0, 18.0]) && !inside([18.0, 0.0], [0.0, 0.0, 18.0, 18.0]));
    }
}

mod bookmarks {
    use crate::bookmarks::{Bookmarks, NAV, grid, pages};

    const IRON: &str = r#"{"bookmarkType":"INGREDIENT","type":"item_stack","ingredient":{"id":"minecraft:iron_ingot","count":1}}"#;

    #[test]
    fn the_bookmark_key_adds_at_the_end_and_takes_off_again() {
        let mut b = Bookmarks::default();
        assert!(b.toggle("minecraft:iron_ingot"));
        assert!(b.toggle("ae2:inscriber"));
        assert_eq!(b.items(), ["minecraft:iron_ingot", "ae2:inscriber"]);
        assert!(!b.toggle("minecraft:iron_ingot"), "a second press removes it");
        assert_eq!(b.items(), ["ae2:inscriber"]);
        assert_eq!(b.remove(0).as_deref(), Some("ae2:inscriber"));
        assert!(b.is_empty() && b.remove(0).is_none());
    }

    #[test]
    fn the_json_file_is_jeis_array_with_its_version_first() {
        let mut b = Bookmarks::default();
        b.toggle("minecraft:iron_ingot");
        assert_eq!(Bookmarks::default().write_json(), "[\n{\"version\":2}\n]");
        assert_eq!(b.write_json(), format!("[\n{{\"version\":2}},\n{IRON}\n]"));
        assert_eq!(Bookmarks::parse_json(&b.write_json()), b);
    }

    #[test]
    fn jeis_file_reads_back_and_what_it_does_not_draw_stays() {
        let recipe = r#"{"bookmarkType":"RECIPE","recipeType":"minecraft:crafting","recipe":{"registryName":"minecraft:stick","a":[1,2,{"b":"]"}]}}"#;
        let sword = r#"{"bookmarkType":"INGREDIENT","type":"item_stack","ingredient":{"id":"minecraft:diamond_sword","count":1,"components":{"minecraft:damage":5}}}"#;
        let fluid = r#"{"bookmarkType":"INGREDIENT","type":"fluid_stack","ingredient":{"id":"minecraft:water"}}"#;
        let text = format!("[\n  {{\"version\":2}},\n  {IRON},\n{recipe},\n{sword},\n{fluid}\n]\n");
        let mut b = Bookmarks::parse_json(&text);
        assert_eq!(b.items(), ["minecraft:iron_ingot", "minecraft:diamond_sword"], "fluids and recipes are not drawn");
        assert_eq!(b.write_json(), format!("[\n{{\"version\":2}},\n{IRON},\n{recipe},\n{sword},\n{fluid}\n]"));
        b.remove(0);
        assert_eq!(b.write_json(), format!("[\n{{\"version\":2}},\n{recipe},\n{sword},\n{fluid}\n]"));
        assert!(Bookmarks::parse_json("not json").is_empty() && Bookmarks::parse_json("").is_empty());
    }

    #[test]
    fn the_legacy_ini_imports_its_item_stacks_once() {
        // `T:` lines were SNBT (`ItemStack.save`), earlier ones JSON; `I:` and `R:` need JEI's registries.
        let ini = "T:{id:\"minecraft:stick\",count:1}\nI:minecraft:water\n\nT:{\"id\":\"minecraft:iron_ingot\",\"count\":1}\nR:{recipe}\n";
        let imported = Bookmarks::import_ini(ini);
        assert_eq!(imported.items(), ["minecraft:stick", "minecraft:iron_ingot"]);
        let mut b = Bookmarks::parse_json(&format!("[{{\"version\":2}},{IRON}]"));
        b.merge(imported);
        assert_eq!(b.items(), ["minecraft:iron_ingot", "minecraft:stick"], "the json's first, no twice");
    }

    #[test]
    fn the_list_fills_its_area_up_to_nine_by_sixteen() {
        let slots = grid([4.0, 10.0, 100.0, NAV + 40.0]);
        assert_eq!(slots.len(), 5 * 2);
        assert_eq!((slots[0], slots[6]), ([4.0, 30.0], [22.0, 48.0]));
        assert_eq!(grid([0.0, 0.0, 400.0, 1000.0]).len(), 9 * 16);
        assert!(grid([0.0, 0.0, 10.0, 100.0]).is_empty());
        assert_eq!((pages(0, 10), pages(10, 10), pages(11, 10), pages(5, 0)), (1, 1, 2, 5));
    }
}
