use std::collections::HashMap;

use log::*;
use screeps::{game, RoomName};
use wasm_bindgen::prelude::*;

mod logging;
mod movement;
mod task;
mod worker;

// tunable important numbers for the bot, in one place for convenience
mod constants {
    // won't do pathing for moving creeps if CPU is above this number
    pub const HIGH_CPU_THRESHOLD: f64 = 250.;
    // won't do pathing for moving creeps if bucket is below this number
    pub const LOW_BUCKET_THRESHOLD: i32 = 1_000;
    // consider creeps to be stuck and get them a new path after this many ticks
    pub const STUCK_REPATH_THRESHOLD: u8 = 10;
    // limits for pathfinder calls
    pub const MAX_OPS: u32 = 100_000;
    pub const MAX_ROOMS: u8 = 64;
    // when task finding fails, idle this long
    pub const NO_TASK_IDLE_TICKS: u32 = 5;
    // builder role considers energy for grabbing above this amount
    pub const BUILDER_ENERGY_PICKUP_THRESHOLD: u32 = 100;
    // builder role considers energy for withdraw from structures above this amount
    pub const BUILDER_ENERGY_WITHDRAW_THRESHOLD: u32 = 1_000;
    // hauler role considers energy for grabbing above this amount
    pub const HAULER_ENERGY_PICKUP_THRESHOLD: u32 = 35;
    // hauler role considers energy for withdraw from structures above this amount
    pub const HAULER_ENERGY_WITHDRAW_THRESHOLD: u32 = 500;
    // fill terminals to this much energy
    pub const TERMINAL_ENERGY_TARGET: u32 = 50_000;
}

// add wasm_bindgen to any function you would like to expose for call from js this one's
// special and must only be called once, so handling for it is carefully managed in main.js
#[wasm_bindgen]
pub fn setup() {
    // show all output of Info level, adjust as needed
    logging::setup_logging(logging::Info);
}

// this is one method of persisting data on the wasm memory heap between ticks
// this is an alternative to keeping state in memory on game objects - but will be lost on
// global resets, which occur at differing frequencies on different server environments
static mut SHARD_STATE: Option<ShardState> = None;

// define the giant struct which holds all of state data we're interested in holding
// for future ticks
pub struct ShardState {
    // the tick when this state was created
    pub global_init_time: u32,
    // owned room states and spawn queues
    pub colony_state: HashMap<RoomName, ColonyState>,
    // workers and their task queues (includes creeps as well as structures)
    pub worker_state: HashMap<worker::WorkerId, worker::WorkerState>,
}

impl Default for ShardState {
    fn default() -> ShardState {
        ShardState {
            global_init_time: game::time(),
            colony_state: HashMap::new(),
            worker_state: HashMap::new(),
        }
    }
}

pub struct ColonyState {
    // todo add stuff here - spawn queue, maybe remote tracking
}

// to use a reserved name as a function name, use `js_name`:
#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
    debug!("loop starting! CPU: {}", game::cpu::get_used());
    let tick = game::time();

    // SAFETY: only one instance of the game loop can be running at a time
    // We must use this same mutable reference throughout the entire tick,
    // as any other access to it would cause undefined behavior!
    let mut shard_state = unsafe { SHARD_STATE.get_or_insert_with(|| ShardState::default()) };

    // register all creeps that aren't yet in our tracking, and delete the state of any that we can
    // no longer see
    worker::scan_and_register_creeps(&mut shard_state);

    // scan for new worker structures as well - every 100 ticks, or if this is the startup tick
    if tick % 100 == 0 || tick == shard_state.global_init_time {
        worker::scan_and_register_structures(&mut shard_state);
    }

    // run all registered workers, attempting to resolve those that haven't already and deleting
    // any workers that don't resolve

    // game state changes like spawning creeps will start happening here, so this is
    // intentionally ordered after we've completed all worker scanning for the tick so we
    // don't need to think about the case of dealing with the object stubs of creeps whose
    // spawn started this tick
    worker::run_workers(&mut shard_state);

    // run movement phase now that all workers have run, while deleting the references to game
    // objects from the current tick (as a way to ensure they aren't used in future ticks
    // as well as to enable them to be GC'd and their memory freed in js heap, if js wants to)
    movement::run_movement_and_remove_worker_refs(&mut shard_state);

    info!(
        "done! cpu: {}, global age {}",
        game::cpu::get_used(),
        game::time() - shard_state.global_init_time
    )
}
