use log::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use screeps::{
    constants::{find, Direction, Part, ResourceType, Terrain},
    enums::StructureObject,
    game,
    local::RoomName,
    objects::{Room, Store, StructureSpawn},
    prelude::*,
};

use crate::{
    constants::*,
    role::WorkerRole,
    task::{Task, TaskQueueEntry},
    worker::Worker,
};

#[derive(Eq, PartialEq, Hash, Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Upgrader {
    #[serde(rename = "r")]
    pub home_room: RoomName,
    #[serde(rename = "i")]
    pub id: u8,
}

impl Worker for Upgrader {
    fn find_task(
        &self,
        store: &Store,
        _worker_roles: &HashSet<WorkerRole>,
        task_reservations: &mut HashMap<Task, u32>,
    ) -> TaskQueueEntry {
        match game::rooms().get(self.home_room) {
            Some(room) => {
                if store.get_used_capacity(Some(ResourceType::Energy)) > 0 {
                    find_upgrade_task(&room, task_reservations)
                } else {
                    let energy_capacity = store
                        .get_free_capacity(Some(ResourceType::Energy))
                        .try_into()
                        .unwrap_or(0);
                    if energy_capacity > 0 {
                        find_energy_or_source(&room, energy_capacity, task_reservations)
                    } else {
                        warn!("no energy capacity!");
                        TaskQueueEntry::new_unreserved(Task::IdleUntil(u32::MAX))
                    }
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
        vec![Move, Move, Carry, Work]
    }
}

fn find_upgrade_task(room: &Room, task_reservations: &mut HashMap<Task, u32>) -> TaskQueueEntry {
    if let Some(controller) = room.controller() {
        TaskQueueEntry::new(Task::Upgrade(controller.id()), 1, task_reservations)
    } else {
        TaskQueueEntry::new_unreserved(Task::IdleUntil(game::time() + NO_TASK_IDLE_TICKS))
    }
}

fn find_energy_or_source(
    room: &Room,
    energy_capacity: u32,
    task_reservations: &mut HashMap<Task, u32>,
) -> TaskQueueEntry {
    // check for energy on the ground of sufficient quantity to care about
    for resource in room.find(find::DROPPED_RESOURCES, None) {
        let resource_amount = resource.amount();
        if resource.resource_type() == ResourceType::Energy
            && resource_amount >= UPGRADER_ENERGY_PICKUP_THRESHOLD
        {
            let reserve_amount = std::cmp::min(resource_amount, energy_capacity);
            let task = Task::TakeFromResource(resource.id());
            if *task_reservations.get(&task).unwrap_or(&0) + reserve_amount <= resource_amount {
                return TaskQueueEntry::new(task, reserve_amount, task_reservations);
            }
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
        if energy_amount >= UPGRADER_ENERGY_WITHDRAW_THRESHOLD {
            let reserve_amount = std::cmp::min(energy_amount, energy_capacity);
            let task = Task::TakeFromStructure(structure.as_structure().id(), ResourceType::Energy);
            if *task_reservations.get(&task).unwrap_or(&0) + reserve_amount <= energy_amount {
                return TaskQueueEntry::new(task, reserve_amount, task_reservations);
            }
        }
    }

    // look for sources with energy we can harvest as a last resort
    for source in room.find(find::SOURCES_ACTIVE, None) {
        let terrain = room.get_terrain();
        let xy = source.pos().xy();
        let mut harvest_positions = 0;
        for direction in enum_iterator::all::<Direction>() {
            if let Some(check_xy) = xy.checked_add_direction(direction) {
                if terrain.get(check_xy.x.u8(), check_xy.y.u8()) != Terrain::Wall {
                    harvest_positions += 1;
                }
            }
        }
        let task = Task::HarvestEnergyUntilFull(source.id());
        if *task_reservations.get(&task).unwrap_or(&0) < harvest_positions {
            return TaskQueueEntry::new(task, 1, task_reservations);
        }
    }

    TaskQueueEntry::new_unreserved(Task::IdleUntil(game::time() + NO_TASK_IDLE_TICKS))
}
