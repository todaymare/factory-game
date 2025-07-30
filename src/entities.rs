use glam::{DVec3, Vec3};
use rand::random;
use sti::{define_key, vec::KVec};

use crate::{constants::DROPPED_ITEM_SCALE, gen_map::KGenMap, items::Item, PhysicsBody, Tick};

define_key!(pub EntityId(u32));


#[derive(Debug)]
pub struct EntityMap {
    pub entities: KGenMap<u32, EntityId, Entity>,
}



#[derive(Debug)]
pub struct Entity {
    pub spawn_tick: Tick,
    pub body: PhysicsBody,
    pub kind: EntityKind,
}



#[derive(Debug)]
#[non_exhaustive]
pub enum EntityKind {
    DroppedItem {
        item: Item,
        is_attracted: bool,
    }
}


impl EntityMap {
    pub fn new() -> Self {
        Self {
            entities: KGenMap::new(),
        }
    }


    pub fn spawn(&mut self, kind: EntityKind, position: DVec3) {
        let entity = Entity {
            spawn_tick: Tick::NEVER,
            body: PhysicsBody {
                position,
                velocity: (random::<Vec3>() - Vec3::ONE*0.5) * kind.splash(),
                aabb_dims: kind.aabb()
            },
            kind,
        };

        self.entities.insert(entity);
    }
}


impl EntityKind {
    pub fn aabb(&self) -> Vec3 {
        match self {
            EntityKind::DroppedItem { .. } => Vec3::splat(DROPPED_ITEM_SCALE),
        }

    }


    pub fn splash(&self) -> f32 {
        match self {
            EntityKind::DroppedItem { .. } => 5.0,
        }

    }


    pub fn dropped_item(item: Item) -> Self {
        Self::DroppedItem { item, is_attracted: false }
    }
}
