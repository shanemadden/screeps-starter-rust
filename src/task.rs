use std::collections::{VecDeque, HashMap};

use serde::{Deserialize, Serialize};

use screeps::{
    constants::ResourceType,
    game,
    local::{ObjectId, Position},
    objects::*,
};

use crate::{
    ShardState,
    movement::{MovementGoal, MovementProfile},
    role::WorkerRole,
    worker::{WorkerId, WorkerReference},
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
    AddTaskToFront(Task),
    CompleteAddTaskToFront(Task),
    CompleteAddTaskToBack(Task),
    DestroyWorker,
}

// #[derive(Eq, PartialEq, Hash, Debug, Copy, Clone, Serialize, Deserialize)]
// pub enum Task {
//     Unreserved(TaskTarget),
//     Simple(TaskTarget, u32),
//     Logistics(TaskTarget, u32, ResourceType),
// }



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


// maybe have a function that determines whether each task type 'reserves' its capacity?
// then it can be taken/dropped as the task is added/removed/cleaned-up-on-death
// or maybe even have the function return a reservation type? (simple, logistics, logistics with rate of change?)

// or do we just do an enum variant like we 'used to' have?

// just use a u32 for everything
// #[derive(Clone, Hash, Debug)]
// pub struct TaskReservation {
//     //pub workers: Vec<WorkerId>,
//     pub worker_count_limit: u8,
//     pub worker_capacity_current: u32,
//     pub worker_capacity_limit: u32,
// }
// how to deal with finding tasks?

// pub enum ReservationType {
//     // limited by total number of active workers
//     WorkerCount,
//     // limited by resource count/capacity
//     ResourceCapacity,
// }

// sooooo
// we need to split this at least a little
// entry on the creep queue should have the task (plus maybe the target) and an optional
// size of the reservation if there is one, so if there's a reservation we know to reduce it
// then the hashmap for tracking hte actual reservation size (or maybe just on the point of interest)
// .. then I need to figure out whether to split the target off of here. maybe the reservation key should be tuple
// of task and target?

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
    ) -> TaskQueueEntry {
        if reservation_amount > 0 {
            // add reservation
        }
        TaskQueueEntry {
            task,
            reservation_amount,
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
                if game::time() >= *tick {
                    TaskResult::Complete
                } else {
                    TaskResult::StillWorking
                }
            }
            Task::MoveToPosition(position, range) => {
                if worker.pos().get_range_to(*position) <= *range {
                    TaskResult::Complete
                } else {
                    TaskResult::MoveMeTo(MovementGoal {
                        pos: *position,
                        range: *range,
                        profile: movement_profile,
                        avoid_creeps: false,
                    })
                }
            }
            // remaining task types are more complex and have handlers
            Task::HarvestEnergyUntilFull(id) => {
                harvest::harvest_energy_until_full(worker, id, movement_profile)
            }
            Task::HarvestEnergyForever(id) => {
                harvest::harvest_energy_forever(worker, id, movement_profile)
            }
            Task::Build(id) => build::build(worker, id, movement_profile),
            Task::Repair(id) => repair::repair(worker, id, movement_profile),
            Task::Upgrade(id) => upgrade::upgrade(worker, id, movement_profile),
            Task::TakeFromResource(id) => {
                logistics::take_from_resource(worker, id, movement_profile)
            }
            Task::TakeFromStructure(id, ty) => {
                logistics::take_from_structure(worker, *id, *ty, movement_profile)
            }
            Task::DeliverToStructure(id, ty) => {
                logistics::deliver_to_structure(worker, *id, *ty, movement_profile)
            }
            Task::SpawnCreep(role) => spawn::spawn_creep(worker, role),
            Task::WaitToSpawn => spawn::wait_to_spawn(worker),
        }
    }
}
