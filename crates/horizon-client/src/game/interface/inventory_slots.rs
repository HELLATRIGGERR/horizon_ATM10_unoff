//! Container screen slots: item and decoration drawing, the carried stack and quick craft drag,
//! slot positions and hit tests, highlights, and the crafting result slot.
use super::*;

#[cfg(test)]
thread_local! { pub(in crate::game) static SLOT_POSITIONS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }

impl Game<'_> {
    pub(super) fn draw_item(&self, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32) {
        // `GuiGraphicsExtractor.item`: the resolved item model (tints, foil and
        // special renderers included), then the decorations.
        self.draw_item_model(batch, stack, x, y);
        self.draw_item_decorations(batch, stack, x, y, None);
    }
    /// `extractSlot` of slot `slot` holding `stack` at (x, y): during a quick
    /// craft drag a dragged slot shows what it would hold over a white
    /// highlight (its count yellow when the slot caps it), and hides while
    /// it is the drag's only slot.
    pub(super) fn draw_slot_item(
        &self,
        batch: &mut HudBatch,
        drag: Option<&DragView>,
        slot: usize,
        stack: &Option<ItemStack>,
        x: f32,
        y: f32,
    ) {
        let carried;
        let (stack, capped) = match drag.and_then(|d| d.slots.get(&slot)) {
            Some(None) => return,
            Some(Some((count, capped))) => {
                batch.fill(&self.gui.atlas, x, y, 16.0, 16.0, rgba(255, 255, 255, 128));
                carried = self
                    .carried
                    .clone()
                    .map(|c| ItemStack { count: *count, ..c });
                (carried.as_ref(), *capped)
            }
            None => (stack.as_ref(), false),
        };
        if let Some(stack) = stack {
            self.draw_item_model(batch, stack, x, y);
            let text = capped.then(|| format!("\u{a7}e{}", stack.count));
            self.draw_item_decorations(batch, stack, x, y, text);
        }
    }
    /// `extractCarriedItem`: the carried stack at the cursor, showing what a
    /// quick craft drag over several slots leaves in hand.
    pub(super) fn draw_carried(&mut self, batch: &mut HudBatch) {
        let Some(mut stack) = self.carried.clone() else {
            return;
        };
        if let Some(left) = self.drag_view().and_then(|d| d.left) {
            if left == 0 {
                return;
            }
            stack.count = left;
        }
        self.draw_item(batch, &stack, self.cursor.0 - 8.0, self.cursor.1 - 8.0);
    }
    /// The quick craft drag in progress, as the menu engine previews it.
    pub(super) fn drag_view(&mut self) -> Option<DragView> {
        let (ty, slots) = self.inv_drag.clone()?;
        let mut view = None;
        self.menu_session(|menu, env, to_menu| {
            let menu_slots: Option<Vec<usize>> = slots.iter().map(|&g| to_menu(g)).collect();
            let shown = match menu.quick_craft_preview(env, ty, &menu_slots?) {
                Some((shown, left)) => {
                    let slots = slots.iter().copied().zip(shown).collect();
                    DragView {
                        slots,
                        left: Some(left),
                    }
                }
                // A single dragged slot is hidden.
                None => DragView {
                    slots: slots.iter().map(|&g| (g, None)).collect(),
                    left: None,
                },
            };
            view = Some(shown);
            Some(false)
        });
        view
    }
    /// Whether a quick craft drag takes `slot`
    /// (`shouldAddSlotToQuickCraft`); screens the menu engine doesn't
    /// model take every slot.
    pub(super) fn quick_craft_accepts(&mut self, slot: usize, ty: i32, dragged: usize) -> bool {
        #[cfg(feature="ae2-lab")]
        if self.ae2_is_fake_slot(slot) {return self.carried.is_some();}
        #[cfg(feature="ae2-lab")]
        if self.ae2_is_remote_repo_slot(slot) {return false;}
        #[cfg(feature="ae2-lab")]
        if self.ae2_local_crafting_terminal_open() {
            if !(slot<36 || (327..=335).contains(&slot)) {return false;}
            let Some(carried)=self.carried.clone() else{return false;};
            if ty!=2 && carried.count as usize<=dragged {return false;}
            let max=self.res.items().stack_max(&carried);
            return self.slot_view(slot).is_none_or(|item|item.same_item_same_components(&carried) && item.count<max);
        }
        #[cfg(feature="ae2-lab")]
        if let Some((owner, _)) = self.remote.as_ref().and_then(|r| r.ae2_terminal_toolbox.as_ref()) {
            if slot == *owner { return false; }
            if (365..374).contains(&slot) {
                let Some(carried) = self.carried.clone() else { return false; };
                if !horizon_client::ae2_network_tool::is_card(&carried.item)
                    || (ty != 2 && carried.count as usize <= dragged) { return false; }
                let max = self.res.items().stack_max(&carried);
                return self.slot_view(slot).is_none_or(|s|
                    s.same_item_same_components(&carried) && s.count < max);
            }
        }
        #[cfg(feature="ae2-lab")]
        if (360..365).contains(&slot) && self.remote.as_ref().is_some_and(|r|r.ae2_view_cells.is_some()) {
            return (ty==2 || dragged==0) && self.carried.as_ref().is_some_and(|s|s.item.as_ref()=="ae2:view_cell" && s.count==1)
                && self.slot_view(slot).is_none();
        }
        let mut accepts = true;
        self.menu_session(|menu, env, to_menu| {
            accepts = to_menu(slot).is_some_and(|s| menu.quick_craft_accepts(env, s, ty, dragged));
            Some(false)
        });
        accepts
    }
    /// `GuiGraphicsExtractor.itemDecorations`: the count (or `count_text`)
    /// and damage bar.
    pub(super) fn draw_item_decorations(
        &self,
        batch: &mut HudBatch,
        stack: &ItemStack,
        x: f32,
        y: f32,
        count_text: Option<String>,
    ) {
        // GuiGraphicsExtractor.itemDecorations, laid out at the origin and
        // moved to (x, y): the damage bar, the cooldown shade and the count.
        let info = self.items().get(&stack.item);
        // ItemStack.isBarVisible: isDamaged, which an unbreakable stack never is.
        let max_damage = if horizon_client::stations::is_damageable(self.items(), stack) {
            stack.max_damage(info)
        } else {
            0
        };
        let cooldown =
            horizon_client::cooldowns::group(stack, info.and_then(|i| i.use_cooldown.as_ref()));
        let partial = self.clock.fraction() as f32;
        // BundleItem overrides the bar with its fullness.
        let mut bar = if info.is_some_and(|item| item.bundle) {
            horizon_client::bundle::Contents::of(self.items(), stack)
                .and_then(|contents| contents.bar())
        } else {
            hud_overlays::damage_bar(stack.effective_damage(info), max_damage)
        };
        #[cfg(feature = "ae2-lab")]
        {
            bar = self.ae2_energy_bar(stack).or(bar);
        }
        // Client-only: a mod may replace or hide the bar vanilla just chose.
        self.native_item_bar(stack, &mut bar);
        let decorations = hud_overlays::ItemDecorations {
            count: stack.count,
            count_text,
            bar,
            cooldown: self.cooldowns.percent(&cooldown, partial),
        };
        let mut draws = vec![hud_layout::GuiDraw::Push(vec![
            hud_layout::PoseOp::Translate(x, y),
        ])];
        draws.extend(hud_overlays::item_decorations(
            0,
            0,
            &decorations,
            &self.gui.atlas.font,
        ));
        draws.push(hud_layout::GuiDraw::Pop);
        hud_layout::paint(batch, &self.gui.atlas, &draws);
        self.native_item_decorations(stack, x, y, batch);
    }
    /// The open container screen's player-inventory origin (`getXOrigin`/
    /// `getYOrigin`): the build's, or the frame's own arithmetic with no
    /// build yet.

    #[cfg(feature="ae2-lab")]
    pub(super) fn ae2_crafting_terminal_screen(&self)->bool {self.remote.as_ref().is_some_and(|r|r.ae2_crafting_screen) || self.ae2_local_crafting_terminal_open()}
    #[cfg(feature="ae2-lab")]
    pub(super) fn ae2_network_tool_screen(&self)->bool {self.remote.as_ref().is_some_and(|r|r.ae2_network_tool_screen)}
    #[cfg(feature="ae2-lab")]
    pub(super) fn ae2_network_status_screen(&self)->bool {self.remote.as_ref().is_some_and(|r|r.ae2_network_status_screen)}
    #[cfg(feature="ae2-lab")]
    pub(super) fn ae2_anchor_screen(&self)->bool {self.remote.as_ref().is_some_and(|r|r.ae2_anchor_screen)}
    #[cfg(feature="ae2-lab")]
    pub(super) fn ae2_craft_screen(&self)->u8 {self.remote.as_ref().map_or_else(||self.ae2_local_craft_screen(),|r|r.ae2_craft_screen)}
    #[cfg(feature="ae2-lab")]
    pub(super) fn ae2_pattern_access_screen(&self)->bool {self.ae2_local_pattern_access_open() || self.remote.as_ref().is_some_and(|r|r.ae2_pattern_access_screen)}
    /// The batch a screen draws in: AE2's tall terminals step the GUI scale down where they
    /// wouldn't fit, as AE2's `AEBaseScreen` does.
    pub(super) fn screen_batch(&self, size: [u32; 2]) -> HudBatch {
        let mut batch = HudBatch::new(size, self.settings.gui_scale_inputs());
        #[cfg(feature="ae2-lab")]
        if ((self.ae2_crafting_terminal_screen() && self.ae2_term_kind().is_none()) || self.ae2_pattern_access_screen()) && batch.height < 251.0 && batch.scale > 1.0 {
            batch.scale -= 1.0;
            batch.width = (size[0] as f32 / batch.scale).ceil();
            batch.height = (size[1] as f32 / batch.scale).ceil();
        }
        batch
    }
    pub(super) fn inventory_origin(&self, batch: &HudBatch) -> (f32, f32) {
        match self.built_container(batch) {
            Some(layout) => layout.origin,
            None => self.inventory_origin_unbuilt(batch),
        }
    }
    /// `inventory_origin` from the game's state alone: the container build's
    /// own arithmetic, and the fallback when no build holds this size. The
    /// open book moves the window right, the room the mods ask beside it
    /// (`screen_room`) left.
    pub(super) fn inventory_origin_unbuilt(&self, batch: &HudBatch) -> (f32, f32) {
        if self.screen == Screen::Creative {
            return Self::creative_origin(batch);
        }
        #[cfg(feature="ae2-lab")]
        if let Some((w,h))=self.ae2_term_size() {return batch.centered_origin(w,h);}
        #[cfg(feature="ae2-lab")]
        if let Some((w,h))=self.ae2x_screen_size() {return batch.centered_origin(w,h);}
        #[cfg(feature="ae2-lab")]
        if self.ae2_crafting_terminal_screen() {return batch.centered_origin(195.0,251.0);}
        #[cfg(feature="ae2-lab")]
        if self.ae2_pattern_access_screen() {return batch.centered_origin(195.0,224.0);}
        #[cfg(feature="ae2-lab")]
        if self.ae2_network_status_screen() {return batch.centered_origin(195.0,157.0);}
        #[cfg(feature="ae2-lab")]
        if self.ae2_anchor_screen() {return batch.centered_origin(195.0,100.0);}
        #[cfg(feature="ae2-lab")]
        if self.ae2_priority_ui() {let (w,h)=self.ae2_priority_size();return batch.centered_origin(w,h);}
        #[cfg(feature="ae2-lab")]
        if let Some((w,h))=self.ae2_part_size() {return batch.centered_origin(w,h);}
        #[cfg(feature="ae2-lab")]
        if self.ae2_craft_screen()==1 {return batch.centered_origin(176.0,107.0);}
        #[cfg(feature="ae2-lab")]
        if self.ae2_craft_screen()==2 {return batch.centered_origin(238.0,206.0);}
        #[cfg(feature="ae2-lab")]
        if self.ae2_craft_screen()==3 {return batch.centered_origin(238.0,185.0);}
        let width = self.native_screen_width().unwrap_or(176.0);
        let (x, y) = batch.centered_origin(width, self.screen_height());
        let x = x + self.recipe_book_shift(batch.width) - self.screen_room_shift(batch);
        (x, y)
    }
    /// InventoryMenu / CraftingMenu slot layouts.
    pub(super) fn slot_position(&self, i: usize) -> (f32, f32) {
        #[cfg(test)]
        SLOT_POSITIONS.with(|count| count.set(count.get() + 1));
        #[cfg(feature="ae2-lab")]
        if let Some(p)=self.ae2_term_slot_position(i) {return p;}
        #[cfg(feature="ae2-lab")]
        if let Some(at) = self.ae2x_slot_position(i) {return at;}
        #[cfg(feature="ae2-lab")]
        if self.remote.as_ref().is_some_and(|r|r.ae2_terminal_toolbox.is_some()) && (365..374).contains(&i) {
            let height = if self.ae2_crafting_terminal_screen() {251.0} else {self.screen_height()};
            return (175.0 + ((i-365)%3) as f32*18.0, height-84.0 + ((i-365)/3) as f32*18.0);
        }
        #[cfg(feature="ae2-lab")]
        if self.remote.as_ref().is_some_and(|r|r.ae2_view_cells.is_some()) && (360..365).contains(&i) {return (if self.ae2_crafting_terminal_screen(){195.0}else{176.0},18.0+(i-360) as f32*18.0);}
        #[cfg(feature="ae2-lab")]
        if self.ae2_pattern_access_screen() {
            return match i {
                0..=8=>(8.0+i as f32*18.0,201.0),
                9..=35=>(8.0+((i-9)%9) as f32*18.0,143.0+((i-9)/9) as f32*18.0),
                300..=353=>(8.0+((i-300)%9) as f32*18.0,17.0+((i-300)/9) as f32*18.0),
                _=>(-1000.0,-1000.0),
            };
        }
        #[cfg(feature="ae2-lab")]
        if self.ae2_priority_ui() {return (-1000.0,-1000.0);}
        #[cfg(feature="ae2-lab")]
        if let Some(place)=self.ae2_part_slot_position(i) {return place;}
        #[cfg(feature="ae2-lab")]
        if self.ae2_craft_screen()!=0 {
            return self.ae2_craft_slot_position(i);
        }
        #[cfg(feature="ae2-lab")]
        if let Some(place)=self.ae2_anchor_slot_position(i) {return place;}
        #[cfg(feature="ae2-lab")]
        if self.ae2_status_open() {return (-1000.0,-1000.0);}
        #[cfg(feature="ae2-lab")]
        if self.ae2_anchor_screen() { return if i==300 {(-18.0,18.0)} else {(-1000.0,-1000.0)}; }
        #[cfg(feature="ae2-lab")]
        if self.ae2_network_status_screen() {
            return match i {
                303..=310 => (13.0+((i-303)%2) as f32*78.0,42.0+((i-303)/2) as f32*18.0),
                325=>(176.0,41.0),326=>(176.0,95.0),_=>(-1000.0,-1000.0),
            };
        }
        #[cfg(feature="ae2-lab")]
        if self.ae2_network_tool_screen() {
            return match i {
                0..=8 => (8.0+i as f32*18.0,142.0),
                9..=35 => (8.0+((i-9)%9) as f32*18.0,84.0+((i-9)/9) as f32*18.0),
                300..=308 => (62.0+((i-300)%3) as f32*18.0,19.0+((i-300)/3) as f32*18.0),
                _ => (-1000.0,-1000.0),
            };
        }
        #[cfg(feature="ae2-lab")]
        if self.ae2_crafting_terminal_screen() {
            return match i {
                0..=8 => (8.0+i as f32*18.0,225.0),
                9..=35 => (8.0+((i-9)%9) as f32*18.0,167.0+((i-9)/9) as f32*18.0),
                300..=326 => (8.0+((i-300)%9) as f32*18.0,18.0+((i-300)/9) as f32*18.0),
                327..=335 => (26.0+((i-327)%3) as f32*18.0,93.0+((i-327)/3) as f32*18.0),
                336 => (134.0,111.0),337 => (81.0,92.0),338 => (99.0,92.0),
                345 => (176.0,18.0),353 => (176.0,54.0),346 => (-18.0,18.0),347 => (-18.0,36.0),349 => (-18.0,54.0),350 => (-18.0,72.0),351 => (-18.0,90.0),352 => (-18.0,108.0),
                _ => (-1000.0,-1000.0),
            };
        }
        if self.creative_inventory_open()
            && let Some(position) = Creative::inventory_slot_position(i)
        {
            return position;
        }
        if self.creative_tab_open()
            && let Some(position) = Creative::tab_slot_position(i)
        {
            return position;
        }
        if let Some(p) = self
            .station_slot_position(i)
            .or_else(|| {
                Self::stock_machine_slot(self.machine_slot_position(i), self.native_screen_shown())
            })
            .or_else(|| self.crafter_slot_position(i))
            .or_else(|| self.mount_slot_position(i))
        {
            return p;
        }
        let table = self.craft_size == 3;
        // ContainerScreen (3 rows) shifts the player inventory down by one pixel.
        // ChestMenu: the inventory sits `18 * (rows - 3) + 1` lower than the
        // 3-row layout of the other menus.
        let shift = match (self.chest_rows(), self.native_screen_inventory_y()) {
            (_, Some(y)) => y - 84.0,
            (Some(rows), None) => 1.0 + (rows as f32 - 3.0) * 18.0,
            (None, None) => 0.0,
        };
        let inventory_x = self.native_screen_inventory_x().unwrap_or(8.0);
        match i {
            0..9 => (inventory_x + i as f32 * 18.0, 142.0 + shift),
            9..36 => (
                inventory_x + ((i - 9) % 9) as f32 * 18.0,
                84.0 + shift + ((i - 9) / 9) as f32 * 18.0,
            ),
            500..504 => (8.0, 8.0 + (i - 500) as f32 * 18.0),
            // InventoryMenu: the shield slot.
            504 => (77.0, 62.0),
            300..500 if let Some(p) = self.native_screen_slot(i - 300) => p,
            300..500 => match self.container_kind() {
                Some(BlockEntity::Furnace(_)) => {
                    [(56.0, 17.0), (56.0, 53.0), (116.0, 35.0)][(i - 300).min(2)]
                }
                _ => (
                    8.0 + ((i - 300) % 9) as f32 * 18.0,
                    18.0 + ((i - 300) / 9) as f32 * 18.0,
                ),
            },
            200 => {
                if table {
                    (124.0, 35.0)
                } else {
                    (154.0, 28.0)
                }
            }
            _ => {
                let g = i - 100;
                let (r, c) = (g / self.craft_size, g % self.craft_size);
                if table {
                    (30.0 + c as f32 * 18.0, 17.0 + r as f32 * 18.0)
                } else {
                    (98.0 + c as f32 * 18.0, 18.0 + r as f32 * 18.0)
                }
            }
        }
    }
    /// The stock hopper/dispenser slot layout, unless a mod screen is shown for
    /// that menu: a mod screen places its own container slots
    /// (`native_screen_slot`) and the player's inventory (`native_screen_inventory_y`),
    /// and this layout answers first in `slot_position`, which kept a mod's screen
    /// on a hopper or a dispenser on the stock grid (#5141). The rest of those
    /// menus is already the mod's: `draw_native_screen` owns the background and
    /// `screen_height` its height.
    pub(super) fn stock_machine_slot(
        stock: Option<(f32, f32)>,
        mod_screen_shown: bool,
    ) -> Option<(f32, f32)> {
        if mod_screen_shown { None } else { stock }
    }

    /// The slot under the cursor (`AbstractContainerScreen.getHoveredSlot`),
    /// from the build's slot boxes rather than a fresh grid, so the hit test
    /// is the one the frame draws with. None while the recipe book covers the
    /// menu in a narrow window (`AbstractRecipeBookScreen.isHovering`).
    pub(super) fn slot_at(&self, batch: &HudBatch, x: f32, y: f32) -> Option<usize> {
        if self.book_covers_menu(batch.width) {
            return None;
        }
        // AE2's priority editor and crafting screens: a press on one of their
        // buttons is a click on a menu entry, and the build's slot boxes do
        // not hold those (the entries' slots sit off the screen).
        #[cfg(feature="ae2-lab")]
        {
            let (ox, oy) = self.inventory_origin(batch);
            if self.ae2_priority_ui() {return self.ae2_priority_entry_at([x-ox,y-oy]).map(|entry|300+entry);}
            if self.ae2_craft_screen()!=0 {
                return self.ae2_craft_entry_at([x-ox,y-oy]);
            }
        }
        match self.built_container(batch) {
            Some(layout) => {
                layout
                    .slots
                    .iter()
                    .find(|(_, r)| {
                        // AbstractContainerScreen.isHovering: the 16x16 slot
                        // grows by one pixel on every side, so the gaps
                        // between slots still hover one.
                        x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
                    })
                    .map(|&(slot, _)| slot)
            }
            None => self.slot_at_unbuilt(batch, x, y),
        }
    }
    /// `slot_at` from the game's state alone, without a build.
    fn slot_at_unbuilt(&self, batch: &HudBatch, x: f32, y: f32) -> Option<usize> {
        let (ox, oy) = self.inventory_origin_unbuilt(batch);
        self.slots().into_iter().find(|&i| {
            let (sx, sy) = self.slot_position(i);
            // AbstractContainerScreen.isHovering: the 16x16 slot grows by one
            // pixel on every side, so the gaps between slots still hover one.
            x >= ox + sx - 1.0 && x < ox + sx + 17.0 && y >= oy + sy - 1.0 && y < oy + sy + 17.0
        })
    }
    /// `AbstractContainerScreen.extractSlotHighlightBack` and
    /// `AbstractContainerScreen.extractSlotHighlightFront`: the 24 x 24
    /// `container/slot_highlight_back` sprite behind the slot at `x`, `y` (its
    /// 16 x 16 item box), the `_front` one over its item.
    pub(super) fn draw_slot_highlight(&self, batch: &mut HudBatch, front: bool, x: f32, y: f32) {
        let sprite = if front {
            "gui/sprites/container/slot_highlight_front"
        } else {
            "gui/sprites/container/slot_highlight_back"
        };
        horizon_client::ui::blit_sprite(
            batch,
            &self.gui.atlas,
            sprite,
            x - 4.0,
            y - 4.0,
            24.0,
            24.0,
            u32::MAX,
        );
    }
    /// The highlight of the slot under the cursor, for the screens whose
    /// slots all sit in `slot_at`'s grid. `Slot.isHighlightable` is false
    /// for the crafter's result (`NonInteractiveResultSlot`).
    pub(super) fn draw_hovered_slot_highlight(
        &self,
        batch: &mut HudBatch,
        front: bool,
        ox: f32,
        oy: f32,
    ) {
        let Some(slot) = self.slot_at(batch, self.cursor.0, self.cursor.1) else {
            return;
        };
        if slot == 309 && matches!(self.container_kind(), Some(BlockEntity::Crafter(_))) {
            return;
        }
        #[cfg(feature="ae2-lab")]
        if self.ae2_term_button_slot(slot) {
            return;
        }
        let (sx, sy) = self.slot_position(slot);
        self.draw_slot_highlight(batch, front, ox + sx, oy + sy);
    }
    pub(super) fn slot_mut(&mut self, i: usize) -> &mut Option<ItemStack> {
        if i >= mount_menu::SADDLE {
            return self.mount_slot_mut(i);
        }
        if i >= stations::INPUT {
            return self.station_slot_mut(i);
        }
        if (500..504).contains(&i) {
            return &mut self.armor[i - 500];
        }
        if i == 504 {
            return &mut self.offhand;
        }
        if i >= 300 {
            return self.container_slot_mut(i - 300);
        }
        if (100..109).contains(&i) {
            &mut self.craft_grid[i - 100]
        } else {
            &mut self.inventory[i]
        }
    }
    /// What a slot shows: the crafting result slots are computed.
    pub(super) fn slot_view(&self, i: usize) -> Option<ItemStack> {
        #[cfg(feature = "ae2-lab")]
        if self.ae2_local_craft_screen()!=0 && (300..327).contains(&i) {
            return self.ae2_local_craft_slot_view(i);
        }
        #[cfg(feature = "ae2-lab")]
        if self.ae2_local_provider_open() && (300..327).contains(&i) {
            return self.ae2_local_provider_slot_view(i);
        }
        #[cfg(feature = "ae2-lab")]
        if self.ae2_local_machine_open() && (300..327).contains(&i) {return self.ae2_local_machine_slot_view(i);}
        #[cfg(feature = "ae2-lab")]
        if self.ae2_local_bus_open() && (300..354).contains(&i) {return self.ae2_local_bus_slot_view(i);}
        #[cfg(feature = "ae2-lab")]
        if self.ae2_local_pattern_access_open() && (300..354).contains(&i) {return self.ae2_local_pattern_access_slot_view(i);}
        #[cfg(feature="ae2-lab")]
        if (365..374).contains(&i) {return self.remote.as_ref().and_then(|r|r.ae2_terminal_toolbox.as_ref()).and_then(|(_, slots)|slots.get(i-365)).cloned().flatten();}
        #[cfg(feature="ae2-lab")]
        if (360..365).contains(&i) {return self.remote.as_ref().and_then(|r|r.ae2_view_cells.as_ref()).and_then(|cells|cells.get(i-360)).cloned().flatten();}
        #[cfg(feature = "ae2-lab")]
        if self.ae2_portable_open() && (300..354).contains(&i) { return self.ae2_portable_slot_view(i); }
        #[cfg(feature = "ae2-lab")]
        if self.ae2_terminal_open() && (300..354).contains(&i) {
            return self.ae2_terminal_slot_view(i);
        }
        match i {
            200 => self.craft_result(),
            309 if matches!(self.container_kind(), Some(BlockEntity::Crafter(_))) => {
                self.crafter_result()
            }
            i if i >= mount_menu::SADDLE => self.mount_slot(i).clone(),
            i if i >= stations::INPUT => self.station_slot(i).clone(),
            500..=503 => self.armor[i - 500].clone(),
            504 => self.offhand.clone(),
            i if i >= 300 => self.container_slot(i - 300).clone(),
            100..=108 => self.craft_grid[i - 100].clone(),
            _ => self.inventory[i].clone(),
        }
    }
    pub(super) fn craft_result(&self) -> Option<ItemStack> {
        // A server's result slot is the server's (`CraftingMenu.slotChangedCraftingGrid`).
        if let Some(r) = &self.remote {
            return r.craft_result.clone();
        }
        self.craft_recipe().map(|(_, result)| result)
    }
    /// The recipe id and result of the crafting grid.
    /// `RecipeCraftingHolder.awardUsedRecipes`: a crafted recipe becomes known
    /// (special ones never do).
    pub(super) fn award_crafted(&mut self, crafts: &[(String, Vec<Option<ItemStack>>)]) {
        for (recipe, ingredients) in crafts {
            // The whole id: the advancements' `recipe_crafted` criteria name
            // `minecraft:x`, and the crafting copy names the recipe bare.
            let recipe = horizon_client::crafting::qualified(recipe);
            self.trigger_recipe_crafted(&recipe, ingredients);
        }
        let recipes: Vec<String> = crafts
            .iter()
            .map(|(r, _)| horizon_client::crafting::qualified(r))
            .collect();
        self.award_recipes(&recipes);
    }
    pub(super) fn craft_recipe(&self) -> Option<(String, ItemStack)> {
        let n = self.craft_size;
        let grid: Vec<Option<ItemStack>> = (0..n * n).map(|i| self.craft_grid[i].clone()).collect();
        let scale = |id: i32| self.maps.data.get(&id).map(|m| m.scale);
        self.recipes()
            .find_with(&self.loop_context, &grid, n, &scale)
            .map(|(name, result)| (name.to_owned(), result))
    }
    pub(super) fn draw_inventory_slots(&mut self, batch: &mut HudBatch, ox: f32, oy: f32) {
        #[cfg(feature="ae2-lab")]
        if self.remote.as_ref().is_some_and(|r|r.ae2_terminal_toolbox.is_some()) && self.ae2_term_kind().is_none() {
            let height = if self.ae2_crafting_terminal_screen() {251.0} else {self.screen_height()};
            batch.sprite_subregion(&self.gui.atlas,"ae2:guis/extra_panels",[ox+174.0,oy+height-90.0,59.0,66.0],[69.0,62.0],128.0,128.0);
        }
        let drag = self.drag_view();
        for i in self.slots() {
            #[cfg(feature="ae2-lab")]
            if self.ae2_network_status_screen() || self.ae2_anchor_screen() || self.ae2_priority_ui() {continue;}
            #[cfg(feature="ae2-lab")]
            if self.ae2_part_open() && self.slot_position(i).0 < -999.0 {continue;}
            #[cfg(feature="ae2-lab")]
            if self.ae2_pattern_access_screen() && !(300..=353).contains(&i) && i>=300 {continue;}
            #[cfg(feature="ae2-lab")]
            if self.ae2_craft_screen()!=0 && (self.ae2_craft_screen()!=1 || i!=300) {continue;}
            #[cfg(feature="ae2-lab")]
            if self.ae2_crafting_terminal_screen() && matches!(i,337|338|345|346|347|349|350|351|353) {
                continue;
            }
            let (sx, sy) = self.slot_position(i);
            #[cfg(feature="ae2-lab")]
            if self.ae2_term_kind().is_some() {
                if sx < -500.0 {continue;}
                if self.ae2_term_draw_slot(batch, i, ox + sx, oy + sy) {continue;}
            }
            #[cfg(feature="ae2-lab")]
            if (360..365).contains(&i) && self.ae2_term_kind().is_none() {
                batch.fill(&self.gui.atlas,ox+sx,oy+sy,18.0,18.0,rgba(65,65,65,255));
                batch.fill(&self.gui.atlas,ox+sx+1.0,oy+sy+1.0,16.0,16.0,rgba(139,139,139,255));
            }
            if i == 504 && self.offhand.is_none() {
                batch.sprite(
                    &self.gui.atlas,
                    "gui/sprites/container/slot/shield",
                    ox + sx,
                    oy + sy,
                );
            }
            if (500..504).contains(&i) && self.armor[i - 500].is_none() {
                let icon = ["helmet", "chestplate", "leggings", "boots"][i - 500];
                batch.sprite(
                    &self.gui.atlas,
                    &format!("gui/sprites/container/slot/{icon}"),
                    ox + sx,
                    oy + sy,
                );
            }
            let stack = self.slot_view(i);
            self.draw_slot_item(batch, drag.as_ref(), i, &stack, ox + sx, oy + sy);
        }
    }
    pub(super) fn draw_inventory(&mut self, batch: &mut HudBatch) {
        screen_info::begin_container("container");
        if self.draw_station(batch) {
            return;
        }
        let (ox, oy) = self.inventory_origin(batch);
        {
            let atlas = &self.gui.atlas;
            self.render_transparent_background("inventory", batch);
            let part = |batch: &mut HudBatch,
                        name: &str,
                        sx: f32,
                        sy: f32,
                        w: f32,
                        h: f32,
                        dx: f32,
                        dy: f32| {
                if let Some(mut r) = atlas.sprites.get(name).copied() {
                    let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
                    r.u0 += sx * du;
                    r.v0 += sy * dv;
                    r.u1 = r.u0 + w * du;
                    r.v1 = r.v0 + h * dv;
                    batch.rect_uv(dx, dy, w, h, r, u32::MAX);
                }
            };
            let grey = rgba(64, 64, 64, 255);
            match self
                .container
                .and_then(|k| self.world.block_entities.get(&k))
            {
                Some(_) if self.draw_native_screen(batch, ox, oy) => {}
                Some(BlockEntity::Chest(_)) if self.draw_machine_screen(batch, ox, oy) => {}
                Some(BlockEntity::Chest(_)) => self.draw_chest_screen(batch, ox, oy),
                #[cfg(feature = "ae2-lab")]
                Some(BlockEntity::Ae2(_)) => self.draw_chest_screen(batch, ox, oy),
                Some(BlockEntity::Furnace(f)) => {
                    let (tex, key, fallback) = match f.kind {
                        horizon_client::containers::FurnaceKind::Furnace => {
                            ("gui/container/furnace", "container.furnace", "Furnace")
                        }
                        horizon_client::containers::FurnaceKind::BlastFurnace => (
                            "gui/container/blast_furnace",
                            "container.blast_furnace",
                            "Blast Furnace",
                        ),
                        horizon_client::containers::FurnaceKind::Smoker => {
                            ("gui/container/smoker", "container.smoker", "Smoker")
                        }
                    };
                    part(batch, tex, 0.0, 0.0, 176.0, 166.0, ox, oy);
                    self.draw_screen_phase(
                        batch,
                        horizon_client::screen_draw::RenderPhase::Background,
                    );
                    // A server's furnace menu has the title it was given.
                    let title = self.remote_menu_title().unwrap_or_else(|| {
                        horizon_client::text::Text::literal(
                            self.container_title(self.translate_with_fallback(key, fallback)),
                        )
                        .resolve_legacy()
                    });
                    screen_info::name_container(|| title.plain());
                    let tw = atlas.font.width_text(&title);
                    batch.text_styled(
                        atlas,
                        &title,
                        ox + (176.0 - tw) / 2.0,
                        oy + 6.0,
                        grey,
                        false,
                    );
                    let inventory =
                        self.translate_with_fallback("container.inventory", "Inventory");
                    batch.text(atlas, &inventory, ox + 8.0, oy + 72.0, grey, false);
                    if f.lit_remaining > 0 {
                        // AbstractFurnaceScreen: flame height ceil(lit * 13) + 1.
                        let total = if f.lit_total == 0 { 200 } else { f.lit_total };
                        let h = ((f.lit_remaining as f32 / total as f32) * 13.0).ceil() + 1.0;
                        part(
                            batch,
                            "gui/sprites/container/furnace/lit_progress",
                            0.0,
                            14.0 - h,
                            14.0,
                            h,
                            ox + 56.0,
                            oy + 36.0 + 14.0 - h,
                        );
                    }
                    if f.cooking_timer > 0 && f.cooking_total > 0 {
                        let w = ((f.cooking_timer as f32 / f.cooking_total as f32) * 24.0).ceil();
                        part(
                            batch,
                            "gui/sprites/container/furnace/burn_progress",
                            0.0,
                            0.0,
                            w,
                            16.0,
                            ox + 79.0,
                            oy + 34.0,
                        );
                    }
                }
                Some(BlockEntity::Brewing(_)) => {}
                Some(BlockEntity::Crafter(_)) => self.draw_crafter_screen(batch, ox, oy),
                Some(
                    BlockEntity::Sign(_)
                    | BlockEntity::Jukebox(_)
                    | BlockEntity::Beacon(_)
                    | BlockEntity::Lectern(_)
                    | BlockEntity::Sculk(_)
                    | BlockEntity::Banner(_)
                    | BlockEntity::CopperGolemStatue(_)
                    | BlockEntity::CreakingHeart(_)
                    | BlockEntity::Spawner(_)
                    | BlockEntity::TrialSpawner(_)
                    | BlockEntity::Vault(_)
                    | BlockEntity::Beehive(_)
                    | BlockEntity::CommandBlock(_)
                    | BlockEntity::EndGateway(_)
                    | BlockEntity::Skull(_)
                    | BlockEntity::DaylightDetector
                    | BlockEntity::Bell
                    | BlockEntity::Conduit(_)
                    | BlockEntity::EndPortal
                    | BlockEntity::EnchantingTable
                    | BlockEntity::Campfire(_)
                    | BlockEntity::ChiseledBookshelf(_)
                    | BlockEntity::Brushable(_)
                    | BlockEntity::Shelf(_)
                    | BlockEntity::Mod(_)
                    | BlockEntity::PotentSulfur(_)
                    | BlockEntity::StructureBlock(_)
                    | BlockEntity::JigsawBlock(_)
                    | BlockEntity::TestBlock(_)
                    | BlockEntity::TestInstanceBlock(_),
                ) => self
                    .draw_screen_phase(batch, horizon_client::screen_draw::RenderPhase::Background),
                #[cfg(feature = "ae2-lab")]
                None if self.ae2_portable_open() => self.draw_chest_screen(batch, ox, oy),
                None if self.mount_active() => self.draw_mount_background(batch, ox, oy),
                None => {
                    let texture = if self.craft_size == 3 {
                        "gui/container/crafting_table"
                    } else {
                        "gui/container/inventory"
                    };
                    part(batch, texture, 0.0, 0.0, 176.0, 166.0, ox, oy);
                    self.draw_screen_phase(
                        batch,
                        horizon_client::screen_draw::RenderPhase::Background,
                    );
                    let title = self.translate_with_fallback("container.crafting", "Crafting");
                    screen_info::name_container(|| title.clone());
                    let tx = if self.craft_size == 3 { 29.0 } else { 97.0 };
                    batch.text(atlas, &title, ox + tx, oy + 6.0, grey, false);
                    if self.craft_size == 3 {
                        let inventory =
                            self.translate_with_fallback("container.inventory", "Inventory");
                        batch.text(atlas, &inventory, ox + 8.0, oy + 72.0, grey, false);
                    }
                }
            }
        }
        #[cfg(feature = "ae2-lab")]
        let portable_open = self.ae2_portable_open();
        #[cfg(not(feature = "ae2-lab"))]
        let portable_open = false;
        if self.container.is_none() && self.craft_size != 3 && !self.mount_active() && !portable_open {
            self.draw_inventory_player(batch, ox, oy);
        }
        self.draw_hovered_slot_highlight(batch, false, ox, oy);
        self.draw_inventory_slots(batch, ox, oy);
        self.draw_hovered_slot_highlight(batch, true, ox, oy);
        self.draw_sort_buttons(batch, ox, oy);
        let book_tip = self.draw_recipe_book(batch);
        let effect_tip = if self.container.is_none() && self.craft_size == 2 && !self.mount_active()
        {
            self.draw_inventory_effects(batch, (ox, oy), 176)
        } else {
            None
        };
        let browser_tip = self.draw_mod_browser(batch);
        self.draw_screen_phase(batch, horizon_client::screen_draw::RenderPhase::Foreground);
        if let Some(lines) = book_tip {
            self.draw_tooltip_lines(batch, &lines, self.cursor.0, self.cursor.1);
        }
        self.draw_carried(batch);
        self.draw_slot_tooltip(batch);
        #[cfg(feature = "ae2-lab")]
        self.draw_pattern_access_header_tooltip(batch,ox,oy);
        self.draw_native_screen_tooltip(batch);
        if let Some(lines) = effect_tip {
            self.draw_tooltip_lines(batch, &lines, self.cursor.0, self.cursor.1);
        }
        if let Some(tooltip) = browser_tip {
            self.draw_mod_browser_tooltip(batch, tooltip);
        }
    }
}

impl Game<'_> {
    /// Slot ids: 0..36 player inventory, 100..109 crafting grid, 200 result.
    pub(super) fn container_kind(&self) -> Option<&BlockEntity> {
        self.container
            .and_then(|k| self.world.block_entities.get(&k))
    }
    /// How many slots `slots` lists, counted without building the list: the
    /// screen render hook asks once a frame and must not allocate. Same
    /// branches in the same order, so the two stay in step.
    pub(super) fn slot_count(&self) -> usize {
        if self.creative_inventory_open() {
            return Creative::inventory_slots().count();
        }
        if self.creative_tab_open() {
            return Creative::tab_slots().count();
        }
        if let Some(n) = self
            .station_slot_count()
            .or_else(|| self.mount_slot_count())
        {
            return n;
        }
        let inventory = 36;
        if let Some(shown) = self.native_screen_slot_count() {
            return inventory + shown;
        }
        let container = match self.container_kind() {
            Some(BlockEntity::Chest(_)) => self.open_container_size(),
            #[cfg(feature = "ae2-lab")]
            Some(BlockEntity::Ae2(_)) => self.open_container_size(),
            Some(BlockEntity::Furnace(_)) => 3,
            Some(BlockEntity::Brewing(_)) => 5,
            Some(BlockEntity::Crafter(_)) => 10,
            Some(
                BlockEntity::Sign(_)
                | BlockEntity::Jukebox(_)
                | BlockEntity::Beacon(_)
                | BlockEntity::Lectern(_)
                | BlockEntity::Sculk(_)
                | BlockEntity::Banner(_)
                | BlockEntity::CopperGolemStatue(_)
                | BlockEntity::CreakingHeart(_)
                | BlockEntity::Spawner(_)
                | BlockEntity::TrialSpawner(_)
                | BlockEntity::Vault(_)
                | BlockEntity::Beehive(_)
                | BlockEntity::CommandBlock(_)
                | BlockEntity::EndGateway(_)
                | BlockEntity::Skull(_)
                | BlockEntity::DaylightDetector
                | BlockEntity::Bell
                | BlockEntity::Conduit(_)
                | BlockEntity::EndPortal
                | BlockEntity::EnchantingTable
                | BlockEntity::Campfire(_)
                | BlockEntity::ChiseledBookshelf(_)
                | BlockEntity::Brushable(_)
                | BlockEntity::Shelf(_)
                | BlockEntity::Mod(_)
                | BlockEntity::PotentSulfur(_)
                | BlockEntity::StructureBlock(_)
                | BlockEntity::JigsawBlock(_)
                | BlockEntity::TestBlock(_)
                | BlockEntity::TestInstanceBlock(_),
            ) => 0,
            None => {
                self.craft_size * self.craft_size + 1 + if self.craft_size == 2 { 5 } else { 0 }
            }
        };
        inventory + container
    }
    /// 300.. are the open container's slots.
    pub(super) fn slots(&self) -> Vec<usize> {
        #[cfg(feature = "ae2-lab")]
        if let Some(slots) = self.ae2x_slots() {return slots;}
        #[cfg(feature = "ae2-lab")]
        if self.ae2_local_provider_open() {
            return (0..36).chain(300..327).collect();
        }
        #[cfg(feature = "ae2-lab")]
        if self.ae2_local_machine_open() {return (0..36).chain(300..327).collect();}
        #[cfg(feature = "ae2-lab")]
        if self.ae2_local_bus_open() {return (0..36).chain(300..354).collect();}
        #[cfg(feature="ae2-lab")]
        if self.ae2_pattern_access_screen() {
            let mut slots:Vec<usize>=(0..36).collect();
            if let Some(rows)=self.ae2_pattern_access_layout().as_ref().and_then(|v|v.get("rows")).and_then(|v|v.as_array()) {
                for (line,row) in rows.iter().take(6).enumerate() {
                    if row.get("kind").and_then(|v|v.as_str())==Some("provider") {slots.extend(300+line*9..309+line*9);}
                }
            }
            return slots;
        }
        #[cfg(feature="ae2-lab")]
        if self.ae2_stock_ui() {return (300..=311).collect();}
        #[cfg(feature="ae2-lab")]
        if self.ae2_priority_ui() {return (300..=308).collect();}
        #[cfg(feature="ae2-lab")]
        if self.ae2_craft_screen()!=0 {return (300..=321).collect();}
        #[cfg(feature="ae2-lab")]
        if self.ae2_network_status_screen() {return (303..311).chain([325,326]).collect();}
        #[cfg(feature="ae2-lab")]
        if self.ae2_anchor_screen() {return vec![300];}
        #[cfg(feature = "ae2-lab")]
        if let Some((view_cells, toolbox)) = self.remote.as_ref().and_then(|r| {
            let view_cells = r.ae2_view_cells.is_some();
            let toolbox = r.ae2_terminal_toolbox.is_some();
            (view_cells || toolbox).then_some((view_cells, toolbox))
        }) {
            return ae2_terminal::terminal_visible_slots(view_cells, toolbox);
        }
        #[cfg(feature = "ae2-lab")]
        if self.ae2_portable_open() { return (0..36).chain(300..354).collect(); }
        #[cfg(feature = "ae2-lab")]
        if self.ae2_terminal_open() {
            let mut slots:Vec<usize>=(0..36).chain(300..354).collect();
            if self.remote.as_ref().is_some_and(|r|r.ae2_view_cells.is_some()) {slots.extend(360..365);}
            if self.remote.as_ref().is_some_and(|r|r.ae2_terminal_toolbox.is_some()) {slots.extend(365..374);}
            return slots;
        }
        if self.creative_inventory_open() {
            return Creative::inventory_slots().collect();
        }
        if self.creative_tab_open() {
            return Creative::tab_slots().collect();
        }
        if let Some(v) = self.station_slots().or_else(|| self.mount_slots()) {
            return v;
        }
        let mut v: Vec<usize> = (0..36).collect();
        if let Some(shown) = self.native_screen_slots() {
            v.extend(shown);
            return v;
        }
        match self.container_kind() {
            Some(BlockEntity::Chest(_)) => {
                v.extend((0..self.open_container_size()).map(|i| 300 + i))
            }
            Some(BlockEntity::Furnace(_)) => v.extend(300..303),
            Some(BlockEntity::Brewing(_)) => v.extend(300..305),
            Some(BlockEntity::Crafter(_)) => v.extend(300..310),
            #[cfg(feature = "ae2-lab")]
            Some(BlockEntity::Ae2(_)) => {
                v.extend((0..self.open_container_size()).map(|i| 300 + i))
            }
            Some(
                BlockEntity::Sign(_)
                | BlockEntity::Jukebox(_)
                | BlockEntity::Beacon(_)
                | BlockEntity::Lectern(_)
                | BlockEntity::Sculk(_)
                | BlockEntity::Banner(_)
                | BlockEntity::CopperGolemStatue(_)
                | BlockEntity::CreakingHeart(_)
                | BlockEntity::Spawner(_)
                | BlockEntity::TrialSpawner(_)
                | BlockEntity::Vault(_)
                | BlockEntity::Beehive(_)
                | BlockEntity::CommandBlock(_)
                | BlockEntity::EndGateway(_)
                | BlockEntity::Skull(_)
                | BlockEntity::DaylightDetector
                | BlockEntity::Bell
                | BlockEntity::Conduit(_)
                | BlockEntity::EndPortal
                | BlockEntity::EnchantingTable
                | BlockEntity::Campfire(_)
                | BlockEntity::ChiseledBookshelf(_)
                | BlockEntity::Brushable(_)
                | BlockEntity::Shelf(_)
                | BlockEntity::Mod(_)
                | BlockEntity::PotentSulfur(_)
                | BlockEntity::StructureBlock(_)
                | BlockEntity::JigsawBlock(_)
                | BlockEntity::TestBlock(_)
                | BlockEntity::TestInstanceBlock(_),
            ) => {}
            None => {
                v.extend((0..self.craft_size * self.craft_size).map(|i| 100 + i));
                v.push(200);
                if self.craft_size == 2 {
                    v.extend(500..505);
                }
            }
        }
        v
    }
}
