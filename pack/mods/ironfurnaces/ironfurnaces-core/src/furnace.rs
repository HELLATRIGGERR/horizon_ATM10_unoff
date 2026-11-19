//! A furnace's cooking: `BlockIronFurnaceTileBase`'s cooking time, fuel and timers, translated from Iron Furnaces
//! 4.3.2 (Apache-2.0). Integer arithmetic is Java's (`int`, truncating division).

/// The slots of the furnace's container: input, fuel, output, then the three augments, then three the screen
/// never shows (the menu is a nine-slot one).
pub const INPUT: usize = 0;
pub const FUEL: usize = 1;
pub const OUTPUT: usize = 2;
pub const AUGMENT_RED: usize = 3;
pub const AUGMENT_GREEN: usize = 4;
pub const AUGMENT_BLUE: usize = 5;
pub const SLOTS: usize = 9;

/// The red augment slot: which recipe type the furnace cooks (`checkRecipeType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Red {
    #[default]
    None,
    Blasting,
    Smoking,
}

impl Red {
    pub fn of_item(item: &str) -> Red {
        match item {
            "ironfurnaces:augment_blasting" => Red::Blasting,
            "ironfurnaces:augment_smoking" => Red::Smoking,
            _ => Red::None,
        }
    }

    /// The recipe type the furnace cooks with.
    pub fn recipe_kind(self) -> &'static str {
        match self {
            Red::None => "minecraft:smelting",
            Red::Blasting => "minecraft:blasting",
            Red::Smoking => "minecraft:smoking",
        }
    }

    /// The block state's `type` (`getStateType`): 1 for smoking, 2 for blasting.
    pub fn state_type(self) -> i32 {
        match self {
            Red::None => 0,
            Red::Smoking => 1,
            Red::Blasting => 2,
        }
    }
}

/// The green augment slot: faster, or slower and thriftier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Green {
    #[default]
    None,
    Speed,
    Fuel,
}

impl Green {
    pub fn of_item(item: &str) -> Green {
        match item {
            "ironfurnaces:augment_speed" => Green::Speed,
            "ironfurnaces:augment_fuel" => Green::Fuel,
            _ => Green::None,
        }
    }
}

/// `getSpeed`: the ticks a cooking takes for a recipe of `recipe_time` ticks, at the tier's `regular` ticks for
/// vanilla's 200: `(int) Math.max(1, regular / (200.0 / recipe_time))`.
pub fn recipe_speed(regular: i32, recipe_time: i32) -> i32 {
    if recipe_time <= 0 {
        return 1;
    }
    let div = 200.0 / f64::from(recipe_time);
    let ticks = f64::from(regular) / div;
    ticks.max(1.0) as i32
}

/// `getCookTime` for an input with a recipe: the speed halved by the speed augment, a quarter longer with the
/// fuel augment.
pub fn cook_time(speed: i32, green: Green) -> i32 {
    let speed = match green {
        Green::None => speed,
        Green::Speed => (speed / 2).max(1),
        Green::Fuel => ((f64::from(speed) * 1.25).ceil() as i32).max(1),
    };
    speed.max(1)
}

/// The burn time a fuel item gives (`furnaceBurnTime`): the item's vanilla burn time scaled to the cooking time
/// of the recipe (`burn * cookTime / 200`), doubled by the fuel augment, halved by the speed augment.
pub fn burn_time(item_burn: i32, cook_time: i32, green: Green) -> i32 {
    let base = item_burn.wrapping_mul(cook_time) / 200;
    match green {
        Green::None => base,
        Green::Fuel => base.wrapping_mul(2),
        Green::Speed => base / 2,
    }
}

/// How many items one cooking finishes (`smeltItem` for 1, `smeltItemMult` for the rest): `div` is the tier's
/// count, and the output stack is held to 64. Returns `(inputs used, items made)`.
pub fn smelt_count(mult: i32, input: i32, output: i32, result: i32) -> (i32, i32) {
    if result <= 0 {
        return (0, 0);
    }
    if mult <= 1 {
        return (1, result);
    }
    let room = (64 - output) / result;
    let used = mult.min(room).min(input).max(0);
    (used, result * used)
}

/// `canSmelt`'s room check: the result fits on the output stack (held to 64).
pub fn fits(output: i32, result: i32, same_item: bool, max_stack: i32) -> bool {
    output == 0 || (same_item && output + result <= max_stack.min(64))
}

/// A furnace's timers, as the block entity keeps them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timers {
    /// `furnaceBurnTime`: ticks of fuel left.
    pub burn: i32,
    /// `cookTime`.
    pub cook: i32,
    /// `totalCookTime`.
    pub total: i32,
    /// `recipesUsed`: what the last fuel was worth, the flame's full height.
    pub used: i32,
}

/// What a tick sees of the slots, and what it asks of them.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub has_input: bool,
    pub has_fuel: bool,
    /// `canSmelt(recipe)`: a recipe for the input and room for its result.
    pub valid: bool,
    /// The fuel item's vanilla burn time.
    pub fuel_burn: i32,
    /// `getCookTime()` with this input and these augments.
    pub cook_time: i32,
    pub green: Green,
}

/// What a tick did to the slots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Done {
    /// A fuel item was taken (`itemstack.shrink(1)` or its remainder).
    pub used_fuel: bool,
    /// The cooking finished: smelt the input.
    pub smelted: bool,
}

impl Timers {
    pub fn burning(&self) -> bool {
        self.burn > 0
    }

    /// One tick of the furnace branch of `BlockIronFurnaceTileBase.tick`: the fuel burns down, a new fuel item
    /// is lit when there is something to cook, the cooking advances and finishes, and cooling when it stopped.
    pub fn tick(&mut self, s: &Step) -> Done {
        let mut done = Done::default();
        if self.burning() {
            self.burn -= 1;
        }
        if self.burning() || (s.has_fuel && s.has_input) {
            if !self.burning() && s.valid {
                self.burn = burn_time(s.fuel_burn, s.cook_time, s.green);
                self.used = self.burn;
                if self.burning() {
                    done.used_fuel = true;
                }
            }
            if self.burning() && s.valid {
                self.cook += 1;
                if self.cook >= self.total {
                    self.cook = 0;
                    self.total = s.cook_time;
                    done.smelted = true;
                }
            } else {
                self.cook = 0;
            }
        } else if !self.burning() && self.cook > 0 {
            self.cook = (self.cook - 2).clamp(0, self.total.max(0));
        }
        done
    }

    /// The menu's four numbers (`getBurnLeftScaled`, `getCookScaled`): the flame's and the arrow's fill, in
    /// pixels of `flame` and `arrow`.
    pub fn flame(&self, pixels: i32) -> i32 {
        let used = if self.used == 0 { 200 } else { self.used };
        self.burn.wrapping_mul(pixels) / used
    }

    pub fn arrow(&self, pixels: i32) -> i32 {
        if self.total == 0 { 0 } else { self.cook.wrapping_mul(pixels) / self.total }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::IronFurnacesConfig, tier::Tier};

    #[test]
    fn a_recipe_of_two_hundred_ticks_takes_the_tiers_speed() {
        let c = IronFurnacesConfig::default_config();
        let times: Vec<_> = Tier::ALL.iter().map(|t| recipe_speed(c.speed(*t), 200)).collect();
        assert_eq!(times, [180, 160, 140, 120, 80, 40, 40, 20, 5, 20, 5, 3, 1]);
    }

    #[test]
    fn other_recipes_scale_with_their_own_time() {
        // A blasting recipe of 100 ticks takes half the tier's ticks, a campfire's 600 three times.
        assert_eq!(recipe_speed(160, 100), 80);
        assert_eq!(recipe_speed(160, 600), 480);
        assert_eq!(recipe_speed(5, 10), 1);
        assert_eq!(recipe_speed(1, 200), 1);
        assert_eq!(recipe_speed(120, 0), 1);
    }

    #[test]
    fn augments_change_the_cooking_time() {
        assert_eq!(cook_time(160, Green::None), 160);
        assert_eq!(cook_time(160, Green::Speed), 80);
        assert_eq!(cook_time(161, Green::Speed), 80);
        assert_eq!(cook_time(1, Green::Speed), 1);
        assert_eq!(cook_time(160, Green::Fuel), 200);
        assert_eq!(cook_time(5, Green::Fuel), 7);
    }

    #[test]
    fn coal_is_worth_what_the_cooking_time_scales_it_to() {
        // Coal burns 1600 ticks in vanilla (eight 200-tick smeltings); an iron furnace's 160 make it 1280.
        assert_eq!(burn_time(1600, 160, Green::None), 1280);
        assert_eq!(burn_time(1600, 160, Green::Fuel), 2560);
        assert_eq!(burn_time(1600, 200, Green::None), 1600);
        assert_eq!(burn_time(1600, 5, Green::None), 40);
        // With an augment the cooking time is already changed: speed halves 160 to 80 and the burn halves again.
        assert_eq!(burn_time(1600, cook_time(160, Green::Speed), Green::Speed), 320);
        // Fuel: 160 becomes 200, and the burn is doubled.
        assert_eq!(burn_time(1600, cook_time(160, Green::Fuel), Green::Fuel), 3200);
        // Eight items of 160 ticks to a coal at the base speed.
        assert_eq!(burn_time(1600, 160, Green::None) / 160, 8);
    }

    #[test]
    fn a_tier_smelts_its_count_at_once() {
        assert_eq!(smelt_count(1, 64, 10, 1), (1, 1));
        assert_eq!(smelt_count(16, 64, 0, 1), (16, 16));
        assert_eq!(smelt_count(16, 5, 0, 1), (5, 5), "only what is there");
        assert_eq!(smelt_count(64, 64, 60, 1), (4, 4), "only what the output holds");
        assert_eq!(smelt_count(32, 64, 0, 2), (32, 64), "two per item");
        assert_eq!(smelt_count(32, 64, 10, 3), (18, 54));
    }

    #[test]
    fn the_output_is_held_to_sixty_four() {
        assert!(fits(0, 1, false, 64));
        assert!(fits(63, 1, true, 64));
        assert!(!fits(64, 1, true, 64));
        assert!(!fits(5, 1, false, 64));
        assert!(fits(15, 1, true, 16));
        assert!(!fits(16, 1, true, 16));
    }

    fn run(timers: &mut Timers, step: Step, ticks: i32) -> (i32, i32) {
        let (mut fuel, mut smelted) = (0, 0);
        for _ in 0..ticks {
            let done = timers.tick(&step);
            fuel += i32::from(done.used_fuel);
            smelted += i32::from(done.smelted);
        }
        (fuel, smelted)
    }

    fn iron(green: Green) -> Step {
        let speed = recipe_speed(160, 200);
        Step { has_input: true, has_fuel: true, valid: true, fuel_burn: 1600, cook_time: cook_time(speed, green), green }
    }

    #[test]
    fn an_iron_furnace_lights_then_smelts_every_160_ticks() {
        let mut t = Timers { total: 160, ..Timers::default() };
        let (fuel, smelted) = run(&mut t, iron(Green::None), 1);
        assert_eq!((fuel, smelted), (1, 0), "the first tick lights the coal");
        assert_eq!((t.burn, t.used, t.cook), (1280, 1280, 1));
        let (_, smelted) = run(&mut t, iron(Green::None), 158);
        assert_eq!(smelted, 0);
        let (_, smelted) = run(&mut t, iron(Green::None), 1);
        assert_eq!(smelted, 1, "the 160th tick of cooking finishes it");
        assert_eq!(t.cook, 0);
        let (fuel, smelted) = run(&mut t, iron(Green::None), 160);
        assert_eq!((fuel, smelted), (0, 1), "the same coal goes on");
        assert!(t.burning());
    }

    #[test]
    fn one_coal_smelts_eight_items_in_an_iron_furnace() {
        let mut t = Timers { total: 160, ..Timers::default() };
        let (fuel, smelted) = run(&mut t, iron(Green::None), 1280);
        assert_eq!(fuel, 1);
        assert_eq!(smelted, 8);
        assert_eq!(t.burn, 1, "the last tick of fuel");
    }

    #[test]
    fn the_speed_augment_halves_the_time_and_uses_the_same_fuel_per_item() {
        let mut t = Timers { total: 80, ..Timers::default() };
        let (_, smelted) = run(&mut t, iron(Green::Speed), 80);
        assert_eq!(smelted, 1);
        assert_eq!(t.used, 320);
        assert_eq!(t.used / 80, 4, "a coal cooks four items with the speed augment, eight without it");
    }

    #[test]
    fn cooling_without_fuel_and_a_missing_recipe_stop_the_cooking() {
        let mut t = Timers { total: 160, cook: 100, burn: 0, used: 0 };
        let idle = Step { has_input: false, has_fuel: false, valid: false, fuel_burn: 0, cook_time: 160, green: Green::None };
        t.tick(&idle);
        assert_eq!(t.cook, 98);
        run(&mut t, idle, 100);
        assert_eq!(t.cook, 0);
        // Burning but nothing valid to cook: the progress resets.
        let mut t = Timers { total: 160, cook: 50, burn: 100, used: 100 };
        t.tick(&Step { valid: false, ..iron(Green::None) });
        assert_eq!(t.cook, 0);
        assert_eq!(t.burn, 99);
    }

    #[test]
    fn the_menus_bars_fill_as_java_scales_them() {
        let t = Timers { burn: 640, cook: 80, total: 160, used: 1280 };
        assert_eq!(t.flame(13), 6);
        assert_eq!(t.arrow(24), 12);
        assert_eq!(Timers::default().arrow(24), 0);
        assert_eq!(Timers { burn: 100, used: 0, ..Timers::default() }.flame(13), 6);
    }
}
