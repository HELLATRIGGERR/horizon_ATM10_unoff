//! `CraftingBehaviour`: the clock every apparatus here shares. An input with a recipe that can run (room for the
//! whole result) and "fuel" (heat, or nothing for the accumulator) advances `progress` one a tick; at `totalTime` it
//! crafts and starts over. Anything missing stops it and resets the progress. `totalTime` is set when the input's
//! item changes (`onInputItemChanged`) and after each craft, from the recipe that fits then (or 100).
use crate::recipe::DEFAULT_TIME;

/// The saved part (`progress`, `totalTime`, both written as shorts) and the part Java keeps in memory
/// (`isProcessing`, sent to clients, and `couldCraftLastTick`, which heat receivers read).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crafting {
    pub progress: i32,
    pub total_time: i32,
    pub processing: bool,
    pub could_craft_last_tick: bool,
}

/// What one tick did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing changed (`stopProcessing` with nothing to stop).
    Idle,
    /// It stopped: progress back to 0 (`sendBlockUpdated`).
    Stopped,
    /// One tick of progress.
    Advanced,
    /// The progress reached `totalTime`: the caller crafts now (`craft(recipe)`).
    Craft,
}

impl Crafting {
    /// `loadAdditional`: `totalTime` was not saved by older versions, so with progress and no total it is read
    /// from the recipe again.
    pub fn loaded(progress: i32, total_time: i32, recipe_time: impl FnOnce() -> i32) -> Self {
        let total_time = if progress > 0 && total_time == 0 { recipe_time() } else { total_time };
        Self { progress, total_time, ..Self::default() }
    }

    /// `tickServer(canProcess, hasInput)`. `can_craft` is `canCraft(getRecipe())`, asked only with input;
    /// `total_time` is `getTotalTime()`, asked when the craft is due (before it consumes the input).
    pub fn tick(&mut self, can_process: bool, has_input: bool, can_craft: impl FnOnce() -> bool, total_time: impl FnOnce() -> i32) -> Step {
        if has_input {
            self.could_craft_last_tick = can_craft();
            if can_process && self.could_craft_last_tick {
                // tryStartProcessing
                if self.progress == 0 {
                    self.processing = true;
                }
                self.progress += 1;
                // tryFinishProcessing
                if self.progress >= self.total_time {
                    self.progress = 0;
                    self.total_time = total_time();
                    return Step::Craft;
                }
                return Step::Advanced;
            }
        }
        self.stop()
    }

    /// `stopProcessing`.
    pub fn stop(&mut self) -> Step {
        if self.progress != 0 || self.processing {
            self.processing = false;
            self.could_craft_last_tick = false;
            self.progress = 0;
            Step::Stopped
        } else {
            Step::Idle
        }
    }

    /// `onInputItemChanged`: a different item (or none) in the input: the total time of what fits now, progress 0.
    pub fn input_changed(&mut self, total_time: i32) {
        self.total_time = total_time;
        self.progress = 0;
    }
}

/// `getTotalTime`: the fitting recipe's time, else `DEFAULT_TIME`.
pub fn total_time(recipe_time: Option<i32>) -> i32 {
    recipe_time.unwrap_or(DEFAULT_TIME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crafts_every_total_time_ticks_while_heated() {
        let mut c = Crafting::default();
        c.input_changed(total_time(Some(100)));
        let mut crafts = Vec::new();
        for t in 1..=250 {
            if c.tick(true, true, || true, || 100) == Step::Craft {
                crafts.push(t);
            }
        }
        assert_eq!(crafts, [100, 200], "a craft on the 100th tick of progress, then again");
        assert!(c.processing && c.could_craft_last_tick);
        assert_eq!(c.progress, 50);
    }

    #[test]
    fn losing_heat_or_input_resets_progress() {
        let mut c = Crafting { total_time: 100, ..Crafting::default() };
        for _ in 0..40 {
            c.tick(true, true, || true, || 100);
        }
        assert_eq!(c.progress, 40);
        assert_eq!(c.tick(false, true, || true, || 100), Step::Stopped, "no heat");
        assert_eq!((c.progress, c.processing, c.could_craft_last_tick), (0, false, false));
        assert_eq!(c.tick(false, true, || true, || 100), Step::Idle);
        // canCraft is still asked without heat: heat receivers read couldCraftLastTick.
        assert!(c.could_craft_last_tick);
        c.tick(true, true, || true, || 100);
        assert_eq!(c.tick(true, false, || unreachable!(), || 100), Step::Stopped, "no input: the recipe isn't looked up");
        let mut asked = false;
        c.tick(true, true, || { asked = true; false }, || 100);
        assert!(asked && c.progress == 0, "no room for the result stops it");
    }

    #[test]
    fn total_time_is_read_when_the_input_changes_and_after_a_craft() {
        let mut c = Crafting::default();
        c.input_changed(total_time(None));
        assert_eq!(c.total_time, DEFAULT_TIME);
        c.input_changed(300);
        for _ in 0..299 {
            assert_eq!(c.tick(true, true, || true, || 7), Step::Advanced);
        }
        assert_eq!(c.tick(true, true, || true, || 7), Step::Craft);
        assert_eq!((c.progress, c.total_time), (0, 7));
        assert_eq!(Crafting::loaded(5, 0, || 60).total_time, 60);
        assert_eq!(Crafting::loaded(0, 0, || 60).total_time, 0);
        assert_eq!(Crafting::loaded(5, 80, || 60).total_time, 80);
    }
}
