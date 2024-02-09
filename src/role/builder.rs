use log::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use screeps::{
    constants::{find, Part, ResourceType},
    enums::StructureObject,
    game,
    local::RoomName,
    objects::{Room, Store, StructureSpawn},
    prelude::*,
};

use crate::{constants::*, role::WorkerRole, task::{Task, TaskQueueEntry}, worker::Worker};

#[derive(Eq, PartialEq, Hash, Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Builder {
    #[serde(rename = "r")]
    pub home_room: RoomName,
    #[serde(rename = "w")]
    pub repair_watermark: u32,
}

impl Worker for Builder {
    fn find_task(&self, store: &Store, _worker_roles: &HashSet<WorkerRole>) -> TaskQueueEntry {
        match game::rooms().get(self.home_room) {
            Some(room) => {
                let energy_amount = store.get_used_capacity(Some(ResourceType::Energy)).try_into().unwrap_or(0);
                if energy_amount > 0 {
                    find_build_or_repair_task(&room, self.repair_watermark, energy_amount)
                } else {
                    let energy_capacity = store.get_free_capacity(Some(ResourceType::Energy)).try_into().unwrap_or(0);
                    find_energy_or_source(&room, energy_capacity)
                }
            }
            None => {
                warn!("couldn't see room for task find, must be an orphan");
                TaskQueueEntry::new_unreserved(Task::IdleUntil(u32::MAX))
            }
        }
    }

    fn get_body_for_creep(&self, _spawn: &StructureSpawn) -> Vec<Part> {
        use Part::*;
        vec![Move, Carry, Work]
    }
}

fn find_build_or_repair_task(room: &Room, repair_watermark: u32, energy_amount: u32) -> TaskQueueEntry {
    // look for repair tasks first
    // note that we're using STRUCTURES instead of MY_STRUCTURES
    // so we can catch roads, containers, and walls
    for structure_object in room.find(find::STRUCTURES, None) {
        // we actually don't care what type of structure this is, convert
        // to the generic `Stucture` which has all we want here
        let structure = structure_object.as_structure();
        let hits = structure.hits();
        let hits_max = structure.hits_max();

        // if hits_max is 0, it's indestructable
        if hits_max != 0 {
            // if the hits are below our 'watermark' to repair to
            // as well as less than half of this struture's max, repair!
            if hits < repair_watermark && hits * 2 < hits_max {
                return TaskQueueEntry::new(Task::Repair(structure.id()), energy_amount);
            }
        }
    }

    // look for construction tasks next
    if let Some(construction_site) = room
        .find(find::MY_CONSTRUCTION_SITES, None)
        .into_iter()
        .next()
    {
        // we can unwrap this id because we know the room the site is in must be visible
        return TaskQueueEntry::new(Task::Build(construction_site.try_id().unwrap()), energy_amount);
    }

    TaskQueueEntry::new_unreserved(Task::IdleUntil(game::time() + NO_TASK_IDLE_TICKS))
}

fn find_energy_or_source(room: &Room, energy_capacity: u32) -> TaskQueueEntry {
    // check for energy on the ground of sufficient quantity to care about
    for resource in room.find(find::DROPPED_RESOURCES, None) {
        let resource_amount = resource.amount();
        if resource.resource_type() == ResourceType::Energy
            && resource.amount() >= BUILDER_ENERGY_PICKUP_THRESHOLD
        {
            let reserve_amount = std::cmp::min(resource_amount, energy_capacity);
            return TaskQueueEntry::new(Task::TakeFromResource(resource.id()), reserve_amount);
        }
    }

    // check structures - filtering for certain types, don't want
    // to have these taking from spawns or extensions!
    for structure in room.find(find::STRUCTURES, None) {
        let store = match &structure {
            StructureObject::StructureContainer(o) => o.store(),
            StructureObject::StructureStorage(o) => o.store(),
            StructureObject::StructureTerminal(o) => o.store(),
            _ => {
                // we don't want to look at this!
                continue;
            }
        };

        let energy_amount = store.get_used_capacity(Some(ResourceType::Energy));
        if energy_amount >= BUILDER_ENERGY_WITHDRAW_THRESHOLD {
            let reserve_amount = std::cmp::min(energy_amount, energy_capacity);
            return TaskQueueEntry::new(Task::TakeFromStructure(structure.as_structure().id(), ResourceType::Energy), reserve_amount);
        }
    }

    // look for sources with energy we can harvest as a last resort
    if let Some(source) = room.find(find::SOURCES_ACTIVE, None).into_iter().next() {
        return TaskQueueEntry::new(Task::HarvestEnergyUntilFull(source.id()), 1);
    }

    TaskQueueEntry::new_unreserved(Task::IdleUntil(game::time() + NO_TASK_IDLE_TICKS))
}
