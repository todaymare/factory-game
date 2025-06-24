use crate::items::{Item, ItemKind};

#[derive(Debug)]
pub struct StructureInventory {
    pub slots: Vec<Option<Item>>,
    pub(super) meta: &'static [SlotMeta],
}


#[derive(PartialEq, Clone, Copy, Debug)]
pub struct SlotMeta {
    pub max_amount: u32,
    pub kind: SlotKind
}



#[derive(PartialEq, Clone, Copy, Debug)]
pub enum SlotKind {
    Input {
        filter: Option<ItemKind>,
    },

    Storage,
    Output,
}


impl StructureInventory {
    pub fn new(meta: &'static [SlotMeta]) -> Self {
        Self {
            slots: vec![None; meta.len()],
            meta,
        }
    }


    pub fn can_accept(&self, mut item: Item) -> bool {
        for index in 0..self.meta.len() {
            let meta = self.meta[index];

            let max_amount = meta.max_amount.min(item.kind.max_stack_size());
            if meta.kind == SlotKind::Output {
                continue;
            }

            if let SlotKind::Input { filter } = meta.kind 
                && let Some(filter) = filter
                && item.kind != filter {
                continue;
            }


            let available = match &self.slots[index] {
                Some(curr_item) => {
                    if curr_item.kind != item.kind { continue }
                    debug_assert!(curr_item.amount <= max_amount);

                    max_amount - curr_item.amount
                },

                None => {
                    max_amount
                },
            };

            item.amount -= available.min(item.amount);

            if item.amount == 0 {
                return true;
            }
        }

        false
    }


    pub fn give_item(&mut self, mut item: Item) {
        debug_assert!(self.can_accept(item));

        for index in 0..self.meta.len() {
            let meta = self.meta[index];
            let max_amount = meta.max_amount.min(item.kind.max_stack_size());

            if meta.kind == SlotKind::Output {
                continue;
            }

            if let SlotKind::Input { filter } = meta.kind 
                && let Some(filter) = filter
                && item.kind != filter {
                continue;
            }


            let slot = &mut self.slots[index];
            match slot {
                Some(curr_item) => {
                    if curr_item.kind != item.kind { continue }
                    debug_assert!(curr_item.amount <= max_amount);

                    let available = max_amount - curr_item.amount;
                    let amount = available.min(item.amount);
                    item.amount -= amount;
                    curr_item.amount += amount;
                },

                None => {
                    let amount = max_amount.min(item.amount);
                    item.amount -= amount;

                    let new_item = item.with_amount(amount);
                    *slot = Some(new_item);
                },
            };


            if item.amount == 0 {
                return;
            }
        }

        unreachable!()
    }


    pub fn inputs_len(&self) -> usize {
        self.meta.iter().filter(|x| matches!(x.kind, SlotKind::Input { .. } | SlotKind::Storage)).count()
    }


    pub fn outputs_len(&self) -> usize {
        self.meta.iter().filter(|x| matches!(x.kind, SlotKind::Output | SlotKind::Storage)).count()
    }


    pub fn input(&self, index: usize) -> (Option<Item>, SlotMeta) {
        debug_assert!(index < self.inputs_len());

        let (i, _) = self.meta.iter()
            .enumerate()
            .filter(|x| matches!(x.1.kind, SlotKind::Input { .. } | SlotKind::Storage))
            .skip(index)
            .next()
            .unwrap();

        (self.slots[i], self.meta[i])
    }


    pub fn output(&self, index: usize) -> (&Option<Item>, SlotMeta) {
        debug_assert!(index < self.outputs_len());

        let (i, _) = self.meta.iter()
            .enumerate()
            .filter(|x| matches!(x.1.kind, SlotKind::Output | SlotKind::Storage))
            .skip(index)
            .next()
            .unwrap();

        (&self.slots[i], self.meta[i])
    }


    pub fn output_mut(&mut self, index: usize) -> &mut Option<Item> {
        debug_assert!(index < self.outputs_len());

        let (i, _) = self.meta.iter()
            .enumerate()
            .filter(|x| matches!(x.1.kind, SlotKind::Output | SlotKind::Storage))
            .skip(index)
            .next()
            .unwrap();

        &mut self.slots[i]
    }


    pub fn try_take(&mut self, index: usize, max: u32) -> Option<Item> {
        let (i, _) = self.meta.iter()
            .enumerate()
            .filter(|x| matches!(x.1.kind, SlotKind::Output | SlotKind::Storage))
            .skip(index)
            .next()
            .unwrap();


        let Some(slot) = &mut self.slots[i]
        else { return None };

        let amount = slot.amount.min(max);
        let item = slot.with_amount(amount);
        slot.amount -= amount;
        if slot.amount == 0 {
            self.slots[i] = None;
        }

        Some(item)
    }
}


impl SlotMeta {
    pub const fn new(max_amount: u32, kind: SlotKind) -> Self {
        Self { max_amount, kind }
    }
}
