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

#[derive(Eq, PartialEq, Hash, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum Task {
    IdleUntil(u32),
    MoveToPosition(Position, u32),
    HarvestEnergyUntilFull(ObjectId<Source>),
    HarvestEnergyForever(ObjectId<Source>),
    Build(ObjectId<ConstructionSite>),
    Repair(ObjectId<Structure>),
    Upgrade(ObjectId<StructureController>),
    TakeFromResource(ObjectId<Resource>),
    TakeFromStructure(ObjectId<Structure>, ResourceType),
    DeliverToStructure(ObjectId<Structure>, ResourceType),
    SpawnCreep(WorkerRole),
    WaitToSpawn,
}
// maybe have a function that determines whether each task type 'reserves' its capacity?
// then it can be taken/dropped as the task is added/removed/cleaned-up-on-death
// or maybe even have the function return a reservation type? (simple, logistics, logistics with rate of change?)

// or do we just do an enum variant like we 'used to' have?

// #[derive(Clone, Hash, Debug)]
// pub struct SharedTaskState {
//     //pub workers: Vec<WorkerId>,
//     pub worker_count_limit: u8,
//     pub worker_capacity_current: u32,
//     pub worker_capacity_limit: u32,
// }
// how to deal with finding tasks?

pub enum ReservationType {
    // no reservations for this task
    None,
    // limited by total number of active workers
    WorkerCount,
    // limited by resource count/capacity
    ResourceCapacity,
}

impl Task {
    // should this be implemented on a related type that represents the point of interest/avail task?
    // need a type where the reservation is 'associated'

    // m,ight be best to have a task type to associate the things, separate struct for
    // the queue entries and the points of interest which include it (and it can have this fn too)

    // so change task to tasktype and add a task struct with a target?
    pub fn get_reservation_type(&self) -> ReservationType {

    }

    pub fn run_task(
        &self,
        worker: &WorkerReference,
        movement_profile: MovementProfile,
    ) -> TaskResult {
        match self {
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
