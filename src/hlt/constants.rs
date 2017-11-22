#![allow(dead_code)]
// Max number of units of distance a ship can travel in a turn
pub const MAX_SPEED: i32 = 7;
// Radius of a ship
pub const SHIP_RADIUS: f64 = 0.5;
// Starting health of ship, also its max
pub const MAX_SHIP_HEALTH: i32 = 255;
// Starting health of ship, also its max
pub const BASE_SHIP_HEALTH: i32 = 255;
// Weapon cooldown period
pub const WEAPON_COOLDOWN: i32 = 1;
// Weapon damage radius
pub const WEAPON_RADIUS: f64 = 5.0;
// Weapon damage
pub const WEAPON_DAMAGE: i32 = 64;
// Radius in which explosions affect other entities
pub const EXPLOSION_RADIUS: f64 = 10.0;
// Distance from the edge of the planet at which ships can try to dock
pub const DOCK_RADIUS: f64 = 4.0;
// Number of turns it takes to dock a ship
pub const DOCK_TURNS: i32 = 5;
// Number of turns it takes to create a ship per docked ship
pub const BASE_PRODUCTIVITY: i32 = 6;
// Total production required to spawn ship
pub const SHIP_COST: i32 = 72;
// Distance from the planets edge at which new ships are created
pub const SPAWN_RADIUS: f64 = 2.0;


// CONFIGURATIONS
pub const MAX_CORRECTIONS: i32 = 180;
pub const FUDGE: f64 = 0.01;

pub const DOCK_PREFERENCE_2P: f64 = 0.60;
pub const RAID_PREFERENCE_2P: f64 = 0.70;
pub const DEFEND_PREFERENCE_2P: f64 = 0.90;
pub const INTERCEPT_PREFERENCE_2P: f64 = 2.80;

pub const DOCK_PREFERENCE_4P: f64 = 0.30;
pub const RAID_PREFERENCE_4P: f64 = 0.70;
pub const DEFEND_PREFERENCE_4P: f64 = 0.80;
pub const INTERCEPT_PREFERENCE_4P: f64 = 3.00;
