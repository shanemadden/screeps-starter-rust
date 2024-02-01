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

impl Task {
    // should this be implemented on a related type that represents the point of interest/avail task?
    // need a type where the reservation is 'associated'

    // m,ight be best to have a task type to associate the things, separate struct for
    // the queue entries and the points of interest which include it (and it can have this fn too)

    // so change task to tasktype and add a task struct with a target?
    // pub fn get_reservation_type(&self) -> Option<ReservationType> {
    //     use ReservationType::*;

    //     match self {
    //         Task::IdleUntil(_) => None,
    //         Task::MoveToPosition(_, _) => None,
    //         Task::HarvestEnergyUntilFull(_) => Some(WorkerCount),
    //         Task::HarvestEnergyForever(_) => Some(WorkerCount),
    //         Task::Build(_) => Some(ResourceCapacity),
    //         Task::Repair(_) => Some(ResourceCapacity),
    //         Task::Upgrade(_) => Some(WorkerCount),
    //         Task::TakeFromResource(_) => Some(ResourceCapacity),
    //         Task::TakeFromStructure(_, _) => Some(ResourceCapacity),
    //         Task::DeliverToStructure(_, _) => Some(ResourceCapacity),
    //         Task::SpawnCreep(_) => Some(WorkerCount),
    //         Task::WaitToSpawn => None,
    //     }
    // }

    // need a function to add reservation -- how do we pass in the capacity when we're doing something like
    // hydrating a reservation from serialization? (as well as when just finding tasks)
    // might need to impl expected store on workers (or at least a store accessor for now?)

    // maybe have the reservation stored inside the task? but that sucks for trying to persist it.. hrm :|

    pub fn add_reservation_with_amount(
        &self,
        shard_state: &mut ShardState,
        amount: u32,
    ) {
        shard_state.reserved_tasks.entry(*self).and_modify(|r| *r = r.saturating_add(amount)).or_insert(amount);
    }

    pub fn add_reservations_for_loaded_queue(
        tasks: VecDeque<Task>,
        shard_state: &mut ShardState,
        worker: &WorkerReference,
    ) -> Option<HashMap<ResourceType, u32>> {
        // if anything needs to touch the store, we'll load it only once
        // nah actually this shoulld be an expected store since it's gonna be updated as we go
        // - make that out of it? pass it back to be set in the worker? bleh
        // will need to add to it also for tasks where we're picking up? blehhhhh too complexxx (and also we still gotta add tracking of
        // what count is reserved in the worker task queue) - maybe should add the 'max' fill amount to the reservation? or maybe having
        // a 'mapped' enum for the reservation? bleh brain's not working -- ok now it's working a bit more, return an expected store hashmap
        // derived from this store _if_ we touch the store during this process
        let mut expected_store = None;

        // TODO from here -- need to implement this
        // also we need to change the worker task queue to store a u32 with it I think, because
        // we can't figure out what the size should be from store when dropping

        // use ReservationType::*;

        // match self {
        //     Task::IdleUntil(_) => None,
        //     Task::MoveToPosition(_, _) => None,
        //     Task::HarvestEnergyUntilFull(_) => Some(WorkerCount),
        //     Task::HarvestEnergyForever(_) => Some(WorkerCount),
        //     Task::Build(_) => Some(ResourceCapacity),
        //     Task::Repair(_) => Some(ResourceCapacity),
        //     Task::Upgrade(_) => Some(WorkerCount),
        //     Task::TakeFromResource(_) => Some(ResourceCapacity),
        //     Task::TakeFromStructure(_, _) => Some(ResourceCapacity),
        //     Task::DeliverToStructure(_, _) => Some(ResourceCapacity),
        //     Task::SpawnCreep(_) => Some(WorkerCount),
        //     Task::WaitToSpawn => None,
        // }


        // and if it's returned then plop this on the worker (and todo add removing expected from the workers during end of tick cleanup)
        expected_store
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
