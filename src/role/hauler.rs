use log::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use screeps::{
    constants::{find, Part, ResourceType},
    enums::StructureObject,
    local::RoomName,
    objects::{Room, Store, Structure, StructureSpawn},
    prelude::*,
};

use crate::{
    constants::*,
    game,
    role::WorkerRole,
    task::{Task, TaskQueueEntry},
    worker::Worker,
};

#[derive(Eq, PartialEq, Hash, Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Hauler {
    #[serde(rename = "r")]
    pub home_room: RoomName,
    #[serde(rename = "i")]
    pub id: u8,
}

impl Worker for Hauler {
    fn find_task(
        &self,
        store: &Store,
        _worker_roles: &HashSet<WorkerRole>,
        task_reservations: &mut HashMap<Task, u32>,
    ) -> TaskQueueEntry {
        match game::rooms().get(self.home_room) {
            Some(room) => {
                let energy_amount = store.get_used_capacity(Some(ResourceType::Energy));
                if energy_amount > 0 {
                    find_delivery_target(&room, energy_amount, task_reservations)
                } else {
                    let energy_capacity = store
                        .get_free_capacity(Some(ResourceType::Energy))
                        .try_into()
                        .unwrap_or(0);
                    if energy_capacity > 0 {
                        find_energy(&room, energy_capacity, task_reservations)
                    } else {
                        warn!("no energy capacity! hurt?");
                        TaskQueueEntry::new_unreserved(Task::IdleUntil(
                            game::time() + NO_TASK_IDLE_TICKS,
                        ))
                    }
                }
            }
            None => {
                warn!("couldn't see room for task find, must be an orphan");
                TaskQueueEntry::new_unreserved(Task::IdleUntil(u32::MAX))
            }
        }
    }

    fn get_body_for_creep(&self, spawn: &StructureSpawn) -> Vec<Part> {
        // scale the creep to larger depending on how much capacity we have available
        let max_energy_avail = spawn
            .room()
            .expect("spawn to have room")
            .energy_capacity_available();
        let multiplier = std::cmp::min(
            max_energy_avail / HAULER_COST_PER_MULTIPLIER,
            HAULER_MAX_MULTIPLIER,
        );

        [Part::Carry, Part::Carry, Part::Move].repeat(multiplier as usize)
    }
}

fn find_energy(
    room: &Room,
    energy_capacity: u32,
    task_reservations: &mut HashMap<Task, u32>,
) -> TaskQueueEntry {
    // check for energy on the ground of sufficient quantity to care about
    for resource in room.find(find::DROPPED_RESOURCES, None) {
        let resource_amount = resource.amount();
        if resource.resource_type() == ResourceType::Energy
            && resource_amount >= HAULER_ENERGY_PICKUP_THRESHOLD
        {
            let reserve_amount = std::cmp::min(resource_amount, energy_capacity);
            let task = Task::TakeFromResource(resource.id());
            if *task_reservations.get(&task).unwrap_or(&0) + reserve_amount <= resource_amount {
                return TaskQueueEntry::new(task, reserve_amount, task_reservations);
            }
        }
    }

    // check structures - containers and terminals only, don't want
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
        if energy_amount >= HAULER_ENERGY_WITHDRAW_THRESHOLD {
            let reserve_amount = std::cmp::min(energy_amount, energy_capacity);
            let task = Task::TakeFromStructure(structure.as_structure().id(), ResourceType::Energy);
            if *task_reservations.get(&task).unwrap_or(&0) + reserve_amount <= energy_amount {
                return TaskQueueEntry::new(task, reserve_amount, task_reservations);
            }
        }
    }

    TaskQueueEntry::new_unreserved(Task::IdleUntil(game::time() + NO_TASK_IDLE_TICKS))
}

fn find_delivery_target(
    room: &Room,
    energy_amount: u32,
    task_reservations: &mut HashMap<Task, u32>,
) -> TaskQueueEntry {
    // check structures - we'll do a pass looking for high priority structures
    // like spawns and extensions and towers before we check terminal and storage -
    // but we'll store their references here as we come accoss them
    let mut maybe_storage = None;
    let mut maybe_terminal = None;

    for structure in room.find(find::STRUCTURES, None) {
        let (store, structure) = match structure {
            // for the three object types that are important to fill, snag their store then cast
            // them right back to StructureObject
            StructureObject::StructureSpawn(ref o) => (o.store(), structure),
            StructureObject::StructureExtension(ref o) => (o.store(), structure),
            StructureObject::StructureTower(ref o) => (o.store(), structure),
            // don't want to look at these types in this iteration, in case
            // one of the covered priority types is later in the vec
            StructureObject::StructureStorage(o) => {
                maybe_storage = Some(o);
                continue;
            }
            StructureObject::StructureTerminal(o) => {
                maybe_terminal = Some(o);
                continue;
            }
            _ => {
                // we don't want to look at this!
                continue;
            }
        };

        let energy_capacity = store
            .get_free_capacity(Some(ResourceType::Energy))
            .try_into()
            .unwrap_or(0);
        if energy_capacity > 0 {
            let reserve_amount = std::cmp::min(energy_amount, energy_capacity);
            let task =
                Task::DeliverToStructure(structure.as_structure().id(), ResourceType::Energy);
            // if it's not already got enough energy on the way, take the job even if we'll overfill
            if *task_reservations.get(&task).unwrap_or(&0) < energy_capacity {
                return TaskQueueEntry::new(task, reserve_amount, task_reservations);
            }
        }
    }

    // check the terminal if we found one
    if let Some(terminal) = maybe_terminal {
        let store = terminal.store();
        if store.get_used_capacity(Some(ResourceType::Energy)) < TERMINAL_ENERGY_TARGET {
            let energy_capacity = store
                .get_free_capacity(Some(ResourceType::Energy))
                .try_into()
                .unwrap_or(0);
            if energy_capacity > 0 {
                let reserve_amount = std::cmp::min(energy_amount, energy_capacity);
                let task = Task::DeliverToStructure(
                    terminal.id().into_type::<Structure>(),
                    ResourceType::Energy,
                );
                if *task_reservations.get(&task).unwrap_or(&0) + reserve_amount <= energy_capacity {
                    return TaskQueueEntry::new(task, reserve_amount, task_reservations);
                }
            }
        }
    }

    // and finally check the storage
    if let Some(storage) = maybe_storage {
        let store = storage.store();
        let energy_capacity = store
            .get_free_capacity(Some(ResourceType::Energy))
            .try_into()
            .unwrap_or(0);
        if energy_capacity > 0 {
            let reserve_amount = std::cmp::min(energy_amount, energy_capacity);
            let task = Task::DeliverToStructure(
                storage.id().into_type::<Structure>(),
                ResourceType::Energy,
            );
            if *task_reservations.get(&task).unwrap_or(&0) + reserve_amount <= energy_capacity {
                return TaskQueueEntry::new(task, reserve_amount, task_reservations);
            }
        }
    }

    TaskQueueEntry::new_unreserved(Task::IdleUntil(game::time() + NO_TASK_IDLE_TICKS))
}
