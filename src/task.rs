use std::collections::{hash_map, HashMap};

use serde::{Deserialize, Serialize};

use screeps::{
    constants::ResourceType,
    game,
    local::{ObjectId, Position},
    objects::*,
};

use crate::{
    movement::{MovementGoal, MovementProfile},
    role::WorkerRole,
    worker::WorkerReference,
};

mod build;
mod harvest;
mod logistics;
mod repair;
mod spawn;
mod upgrade;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum TaskResult {
    Complete,
    StillWorking,
    MoveMeTo(MovementGoal),
    AddTaskToFront(TaskQueueEntry),
    CompleteAddTaskToFront(TaskQueueEntry),
    CompleteAddTaskToBack(TaskQueueEntry),
    DestroyWorker,
}

#[derive(Eq, PartialEq, Hash, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum Task {
    // no reservation
    WaitToSpawn,
    IdleUntil(u32),
    MoveToPosition(Position, u32),
    // simple reservation
    HarvestEnergyUntilFull(ObjectId<Source>),
    HarvestEnergyForever(ObjectId<Source>),
    Upgrade(ObjectId<StructureController>),
    SpawnCreep(WorkerRole),
    // logistic reservation
    Build(ObjectId<ConstructionSite>),
    Repair(ObjectId<Structure>),
    TakeFromResource(ObjectId<Resource>),
    TakeFromStructure(ObjectId<Structure>, ResourceType),
    DeliverToStructure(ObjectId<Structure>, ResourceType),
}

#[derive(Eq, PartialEq, Hash, Debug, Copy, Clone, Serialize, Deserialize)]
pub struct TaskQueueEntry {
    #[serde(rename = "t")]
    task: Task,
    #[serde(rename = "r")]
    reservation_amount: u32,
}

impl TaskQueueEntry {
    pub fn new_unreserved(task: Task) -> TaskQueueEntry {
        TaskQueueEntry {
            task,
            reservation_amount: 0,
        }
    }

    pub fn new(
        task: Task,
        reservation_amount: u32,
        task_reservations: &mut HashMap<Task, u32>,
    ) -> TaskQueueEntry {
        if reservation_amount > 0 {
            task_reservations
                .entry(task)
                .and_modify(|r| *r = r.saturating_add(reservation_amount))
                .or_insert(reservation_amount);
        }
        TaskQueueEntry {
            task,
            reservation_amount,
        }
    }

    pub fn remove_reservation(&self, task_reservations: &mut HashMap<Task, u32>) {
        if self.reservation_amount > 0 {
            if let hash_map::Entry::Occupied(mut o) = task_reservations.entry(self.task) {
                // move the above modify logic into here so we dont hash twice
                *o.get_mut() = o.get().saturating_sub(self.reservation_amount);

                if *o.get() == 0 {
                    o.remove();
                }
            }
        }
    }

    pub fn run_task(
        &self,
        worker: &WorkerReference,
        movement_profile: MovementProfile,
    ) -> TaskResult {
        match self.task {
            // idle worker, let's just deal with that directly
            Task::IdleUntil(tick) => {
                if game::time() >= tick {
                    TaskResult::Complete
                } else {
                    TaskResult::StillWorking
                }
            }
            Task::MoveToPosition(position, range) => {
                if worker.pos().get_range_to(position) <= range {
                    TaskResult::Complete
                } else {
                    TaskResult::MoveMeTo(MovementGoal {
                        pos: position,
                        range,
                        profile: movement_profile,
                        avoid_creeps: false,
                    })
                }
            }
            // remaining task types are more complex and have handlers
            Task::HarvestEnergyUntilFull(id) => {
                harvest::harvest_energy_until_full(worker, &id, movement_profile)
            }
            Task::HarvestEnergyForever(id) => {
                harvest::harvest_energy_forever(worker, &id, movement_profile)
            }
            Task::Build(id) => build::build(worker, &id, movement_profile),
            Task::Repair(id) => repair::repair(worker, &id, movement_profile),
            Task::Upgrade(id) => upgrade::upgrade(worker, &id, movement_profile),
            Task::TakeFromResource(id) => {
                logistics::take_from_resource(worker, &id, movement_profile)
            }
            Task::TakeFromStructure(id, ty) => {
                logistics::take_from_structure(worker, &id, ty, movement_profile)
            }
            Task::DeliverToStructure(id, ty) => {
                logistics::deliver_to_structure(worker, &id, ty, movement_profile)
            }
            Task::SpawnCreep(role) => spawn::spawn_creep(worker, &role),
            Task::WaitToSpawn => spawn::wait_to_spawn(worker),
        }
    }
}
