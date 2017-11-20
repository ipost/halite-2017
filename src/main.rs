/* This is a Rust implementation of the Settler starter bot for Halite-II
 * For the most part, the code is organized like the Python version, so see
 * that
 * code for more information.
 * */

#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

mod hlt;

use hlt::entity::{DockingStatus, Entity, GameState, Planet, Position, Ship};
use hlt::game::Game;
use hlt::logging::Logger;
use hlt::command::Command;
use hlt::macros::*;
use std::collections::HashMap;
macro_rules! assert_unreachable (
    () => { panic!(format!("line {}", line!())) }
    );
use hlt::game_map::GameMap;
use hlt::constants::{DEFEND_PREFERENCE_2P, DEFEND_PREFERENCE_4P, DOCK_PREFERENCE_2P, DOCK_PREFERENCE_4P,
                     INTERCEPT_PREFERENCE_2P, INTERCEPT_PREFERENCE_4P, RAID_PREFERENCE_2P, RAID_PREFERENCE_4P,
                     DOCK_RADIUS, DOCK_TURNS, MAX_CORRECTIONS, MAX_SPEED, WEAPON_RADIUS};
extern crate time;
use time::PreciseTime;
use std::cmp::Ordering;

struct Targets<'a> {
    docked_ships: Vec<&'a Ship>,
    undocked_ships: Vec<&'a Ship>,
    planets: Vec<&'a Planet>,
}

enum Target<'a> {
    Ship(&'a Ship),
    Planet(&'a Planet),
}

impl<'a> Targets<'a> {
    pub fn closest<T: Entity>(&self, ent: T) -> Target {
        Target::Ship(self.docked_ships[0])
    }
}

#[derive(Debug)]
enum Move<'a> {
    DockMove(&'a Planet, f64),
    RaidMove(&'a Ship, f64),
    DefendMove(&'a Ship, f64),
    InterceptMove(&'a Ship, f64),
}

impl<'a> Move<'a> {
    pub fn value(&self) -> f64 {
        match self {
            &Move::DockMove(p, v) => v,
            &Move::RaidMove(s, v) => v,
            &Move::DefendMove(s, v) => v,
            &Move::InterceptMove(s, v) => v,
        }
    }

    pub fn id(&self) -> i32 {
        match self {
            &Move::DockMove(p, v) => p.id,
            &Move::RaidMove(s, v) => s.id,
            &Move::DefendMove(s, v) => s.id,
            &Move::InterceptMove(s, v) => s.id,
        }
    }

    pub fn commitment(&self) -> i32 {
        match self {
            //&Move::DockMove(p, v) => p.committed_ships.get(),
            &Move::DockMove(p, v) => 0,
            &Move::RaidMove(s, v) => s.committed_ships.get(),
            &Move::DefendMove(s, v) => s.committed_ships.get(),

            // do not intercept until we defend and raid
            &Move::InterceptMove(s, v) => s.committed_ships.get() + 1,
        }
    }

    pub fn recalculate(&self, ship: &Ship, game_map: &GameMap) -> Move {
        let (dock_preference, RAID_PREFERENCE, defend_preference, intercept_preference) =
            if game_map.state.players.len() > 2 {
                (
                    DOCK_PREFERENCE_4P,
                    RAID_PREFERENCE_4P,
                    DEFEND_PREFERENCE_4P,
                    INTERCEPT_PREFERENCE_4P,
                )
            } else {
                (
                    DOCK_PREFERENCE_2P,
                    RAID_PREFERENCE_2P,
                    DEFEND_PREFERENCE_2P,
                    INTERCEPT_PREFERENCE_2P,
                )
            };
        match self {
            &Move::DockMove(p, v) => Move::DockMove(p, dock_preference * ship.dock_value(p, game_map)),
            &Move::RaidMove(s, v) => Move::RaidMove(s, RAID_PREFERENCE * ship.raid_value(s)),
            &Move::DefendMove(s, v) => Move::DefendMove(s, defend_preference * ship.defense_value(s, &game_map)),
            &Move::InterceptMove(s, v) => Move::InterceptMove(s, intercept_preference * ship.intercept_value(s)),
        }
    }
}

#[derive(Debug)]
struct ShipMoves<'a> {
    ship: &'a Ship,
    dock_moves: Vec<Move<'a>>,
    raid_moves: Vec<Move<'a>>,
    defend_moves: Vec<Move<'a>>,
    intercept_moves: Vec<Move<'a>>,
    best_move: usize,
}

impl<'a> ShipMoves<'a> {
    // moves must be sorted by value within their type
    pub fn update_best_move(&mut self) {
        match self.best_move {
            0 => self.dock_moves.remove(0),
            1 => self.raid_moves.remove(0),
            2 => self.defend_moves.remove(0),
            3 => self.intercept_moves.remove(0),
            _ => assert_unreachable!(),
        };
        self.refresh_best_move();
    }

    pub fn refresh_best_move(&mut self) {
        self.sort_moves();
        self.best_move = {
            let mut moves: Vec<Option<&Move>> = Vec::with_capacity(3);
            moves.push(self.dock_moves.first());
            moves.push(self.raid_moves.first());
            moves.push(self.defend_moves.first());
            moves.push(self.intercept_moves.first());

            let best_move = moves
                .into_iter()
                .filter(|m| m.is_some())
                .map(|m| m.unwrap())
                .min_by(|move1, move2| {
                    move1.value().partial_cmp(&move2.value()).unwrap()
                });

            match best_move.unwrap() {
                &Move::DockMove(p, v) => 0,
                &Move::RaidMove(s, v) => 1,
                &Move::DefendMove(s, v) => 2,
                &Move::InterceptMove(s, v) => 3,
            }
        };
    }

    fn sort_moves(&mut self) {
        self.dock_moves.sort_by(|dock_move1, dock_move2| {
            let dm = match dock_move1 {
                &Move::DockMove(p, v) => v,
                _ => assert_unreachable!(),
            }.partial_cmp(&match dock_move2 {
                &Move::DockMove(p, v) => v,
                _ => assert_unreachable!(),
            });
            dm.unwrap()
        });

        self.raid_moves.sort_by(|raid_move1, raid_move2| {
            let (ship1, v1) = match raid_move1 {
                &Move::RaidMove(s, v) => (s, v),
                _ => assert_unreachable!(),
            };
            let (ship2, v2) = match raid_move2 {
                &Move::RaidMove(s, v) => (s, v),
                _ => assert_unreachable!(),
            };
            v1.partial_cmp(&v2).unwrap()
        });

        self.defend_moves.sort_by(|defend_move1, defend_move2| {
            let (ship1, v1) = match defend_move1 {
                &Move::DefendMove(s, v) => (s, v),
                _ => assert_unreachable!(),
            };
            let (ship2, v2) = match defend_move2 {
                &Move::DefendMove(s, v) => (s, v),
                _ => assert_unreachable!(),
            };
            v1.partial_cmp(&v2).unwrap()
        });

        self.intercept_moves
            .sort_by(|intercept_move1, intercept_move2| {
                let (ship1, v1) = match intercept_move1 {
                    &Move::InterceptMove(s, v) => (s, v),
                    _ => assert_unreachable!(),
                };
                let (ship2, v2) = match intercept_move2 {
                    &Move::InterceptMove(s, v) => (s, v),
                    _ => assert_unreachable!(),
                };
                v1.partial_cmp(&v2).unwrap()
            });
    }

    pub fn remaining_moves(&self) -> usize {
        self.dock_moves.len() + self.raid_moves.len() + self.defend_moves.len() + self.intercept_moves.len()
    }

    pub fn best_move(&self) -> &Move {
        match self.best_move {
            0 => self.dock_moves.first().unwrap(),
            1 => self.raid_moves.first().unwrap(),
            2 => self.defend_moves.first().unwrap(),
            3 => self.intercept_moves.first().unwrap(),
            _ => assert_unreachable!(),
        }
    }

    pub fn as_string(&self) -> String {
        format!(
            "
ShipMoves {{
    ship_id: {}
    best_move: {:#?}
    dock_moves: {}
    raid_moves: {}
    defend_moves: {}
    intercept_moves: {}
    }}",
            self.ship.id,
            self.best_move(),
            self.dock_moves
                .iter()
                .map(|m| format!("planet_id: {}, value: {}", m.id(), m.value()))
                .fold(String::new(), |acc, s| { acc + "\n        " + &s }),
            self.raid_moves
                .iter()
                .map(|m| format!("ship_id: {}, value: {}", m.id(), m.value()))
                .fold(String::new(), |acc, s| { acc + "\n        " + &s }),
            self.defend_moves
                .iter()
                .map(|m| format!("ship_id: {}, value: {}", m.id(), m.value()))
                .fold(String::new(), |acc, s| { acc + "\n        " + &s }),
            self.intercept_moves
                .iter()
                .map(|m| format!("ship_id: {}, value: {}", m.id(), m.value()))
                .fold(String::new(), |acc, s| { acc + "\n        " + &s }),
        )
    }
}

fn main() {
    // Initialize the game
    let bot_name = "memetron_420v4";
    let game = Game::new(bot_name);
    // Initialize logging
    let mut logger = Logger::new(game.my_id);
    logger.log(&format!("Starting my {} bot!", bot_name));

    // For each turn
    let mut turn_number: usize = 0;
    let gs = GameState {
        players: vec![],
        planets: vec![],
    };
    let mut game_map = GameMap::new(&game, gs);
    loop {
        let start_time = PreciseTime::now();
        turn_number = turn_number + 1;
        // Update the game state
        game_map = game.update_map(game_map);
        let mut command_queue: Vec<Command> = Vec::new();

        // set playercount-dependent params
        let (dock_preference, RAID_PREFERENCE, defend_preference, intercept_preference) =
            if game_map.state.players.len() > 2 {
                (
                    DOCK_PREFERENCE_4P,
                    RAID_PREFERENCE_4P,
                    DEFEND_PREFERENCE_4P,
                    INTERCEPT_PREFERENCE_4P,
                )
            } else {
                (
                    DOCK_PREFERENCE_2P,
                    RAID_PREFERENCE_2P,
                    DEFEND_PREFERENCE_2P,
                    INTERCEPT_PREFERENCE_2P,
                )
            };

        // Loop over all of our player's ships
        let ships = game_map.get_me().all_ships();
        {
            let ship_ids = ships
                .iter()
                .map(|s| s.id.to_string())
                .collect::<Vec<String>>()
                .join(" ");
            logger.log(&format!("turn {}, my ships: {}", turn_number, ship_ids));
        }

        // TODO: prefer planets with at least 3 ports and farther from the enemy on
        // turn one. Also consider how near other planets are--don't want to have
        // nothing nearby
        // for quick expansion"

        let planets_to_dock: Vec<&Planet> = game_map
            .all_planets()
            .iter()
            .filter(|p| {
                !p.is_owned() || (p.is_owned() && p.owner.unwrap() == game.my_id as i32 && p.open_docks() > 0)
            })
            .collect();

        let mut enemy_docked_ships: Vec<&Ship> = game_map
            .enemy_ships()
            .into_iter()
            .filter(|s| !s.is_undocked())
            .collect();

        let enemy_undocked_ships: Vec<&Ship> = game_map
            .enemy_ships()
            .into_iter()
            .filter(|s| s.is_undocked())
            .collect();

        // predict enemy ship movement
        let my_ships = game_map.my_ships();
        for s in enemy_undocked_ships.iter() {
            let my_closest = s.nearest_entity(my_ships.as_slice());
            let (speed, angle) = s.route_to(my_closest, &game_map);
            let velocity_x = speed as f64 * (angle as f64).to_radians().cos();
            let velocity_y = speed as f64 * (angle as f64).to_radians().sin();
            s.set_velocity(velocity_x, velocity_y);
        }

        let ship_advantage = my_ships.len() as f64 / game_map.enemy_ships().len() as f64;
        let my_docked_ships: Vec<&Ship> = my_ships.into_iter().filter(|s| !s.is_undocked()).collect();

        let mut ships_to_order = vec![];
        let mut attempted_commands: HashMap<i32, i32> = HashMap::new();
        // Ignore ships that are docked or in the process of (un)docking
        for ship in ships {
            if ship.docking_status == DockingStatus::UNDOCKED {
                attempted_commands.insert(ship.id, 0);
                ships_to_order.push(ship);
            } else {
                logger.log(&format!(
                    "  ship {} will remain {}",
                    ship.id,
                    ship.docking_status
                ));
                ship.command.set(Some(Command::Stay()));
            }
        }

        let mut commands_issued = 0;
        let mut break_command = -1;
        while game_map.my_ships().iter().any(|s| !s.commanded()) && break_command != commands_issued {
            break_command = commands_issued;
            // recalculating ship_moves after a command is issued allows me to update
            // values in
            // response to commands being issued -- meaning commitment can be a factor, so
            // no more
            // sorting by commitment and then value hmmm
            // have to figure out how to omit moves which have been ruled out
            let mut all_ship_moves: Vec<ShipMoves> = game_map
                .my_ships()
                .into_iter()
                .filter(|s| !s.commanded() && s.is_undocked())
                .map(|ship| {
                    let mut dock_moves: Vec<Move> = planets_to_dock
                        .iter()
                        .map(|p| {
                            Move::DockMove(*p, dock_preference * ship.dock_value(p, &game_map))
                        })
                        .collect();
                    let mut raid_moves: Vec<Move> = enemy_docked_ships
                        .iter()
                        .map(|enemy_ship| {
                            Move::RaidMove(*enemy_ship, RAID_PREFERENCE * ship.raid_value(enemy_ship))
                        })
                        .collect();
                    let mut defend_moves: Vec<Move> = enemy_undocked_ships
                        .iter()
                        .map(|enemy_ship| {
                            Move::DefendMove(
                                *enemy_ship,
                                defend_preference * ship.defense_value(enemy_ship, &game_map),
                            )
                        })
                        .collect();
                    let mut intercept_moves: Vec<Move> = enemy_undocked_ships
                        .iter()
                        .map(|enemy_ship| {
                            Move::InterceptMove(
                                *enemy_ship,
                                intercept_preference * ship.intercept_value(enemy_ship),
                            )
                        })
                        .collect();
                    let best_move = 4;
                    let mut s_m = ShipMoves {
                        ship,
                        dock_moves,
                        raid_moves,
                        defend_moves,
                        intercept_moves,
                        best_move,
                    };
                    s_m.refresh_best_move();
                    s_m
                })
                .collect();

            // break executed at end if command issued
            while true {
                let (ship_id, command) = {
                    // find the current ship which has the best move to make
                    let ship_to_move = all_ship_moves
                        .iter()
                        .filter(|s_m| s_m.remaining_moves() > 1)
                        .min_by(|s_m1, s_m2| {
                            s_m1.best_move()
                                .value()
                                .partial_cmp(&s_m2.best_move().value())
                                .unwrap()
                        });
                    if ship_to_move.is_none() {
                        // all ships_to_move are out of possible moves
                        break;
                    }
                    // logger.log(&format!(
                    //     "ship_to_move: {}",
                    //     ship_to_move.unwrap().as_string()
                    // ));
                    let mut ship_to_move = ship_to_move.unwrap();
                    let ship = ship_to_move.ship;
                    match ship_to_move.best_move() {
                        // execute dock move
                        &Move::DockMove(planet, v) => {
                            let destination = &ship.closest_point_to(planet, 3.0);
                            // check if nearby enemies with commitment == 0
                            // TODO: maybe move this to dock_value
                            let nearby_enemies = enemy_undocked_ships.iter().any(|e_s| {
                                e_s.distance_to(destination) < (DOCK_TURNS * MAX_SPEED * 2) as f64
                                    && e_s.committed_ships.get() == 0
                            });

                            // if all dock spots are claimed no command
                            // maybe move this to dock_value
                            if (planet.num_docking_spots
                                - (planet.committed_ships.get() + planet.docked_ships.len() as i32))
                                == 0
                            //|| nearby_enemies
                            {
                                (ship.id, None)

                            // if close enough to dock
                            } else if (planet.turns_until_spawn() as f64)
                                < (ship.distance_to_surface(planet) + DOCK_RADIUS) / MAX_SPEED as f64
                            {
                                (ship.id, None)
                            } else if ship.in_dock_range(planet) {
                                planet.committed_ships.set(planet.committed_ships.get() + 1);
                                logger.log(&format!("  Ship {} docking to {}", ship.id, planet.id));
                                (ship.id, Some(ship.dock(planet)))

                            // otherwise, fly towards planet
                            } else {
                                let destination = &ship.closest_point_to(planet, 3.0);
                                let (speed, angle) = ship.route_to(destination, &game_map);
                                let speed_angle: Option<(i32, i32)> =
                                    ship.safely_adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS);
                                match speed_angle {
                                    Some((speed, angle)) => {
                                        logger.log(&format!(
                                            "  ship {} : speed: {}, angle: {}, target: {}, target planet: {}",
                                            ship.id,
                                            speed,
                                            angle,
                                            destination,
                                            planet.id
                                        ));
                                        planet.increment_committed_ships();
                                        (ship.id, Some(ship.thrust(speed, angle)))
                                    }
                                    _ => {
                                        logger.log(&format!(
                                            "  --- failed to find path to planet {} for ship {}",
                                            planet.id,
                                            ship.id
                                        ));
                                        (ship.id, None)
                                    }
                                }
                            }
                        }

                        // execute raid move
                        &Move::RaidMove(enemy_ship, v) => if ship.distance_to(enemy_ship) < WEAPON_RADIUS / 2.0 {
                            // TODO: run away when attacked?
                            logger.log(&format!(
                                "  ship {} will remain {} to attack {}",
                                ship.id,
                                ship.docking_status,
                                enemy_ship.id
                            ));
                            (ship.id, Some(Command::Stay()))
                        } else {
                            let destination = &ship.closest_point_to(enemy_ship, WEAPON_RADIUS);
                            let (speed, angle) = ship.route_to(destination, &game_map);
                            let speed_angle: Option<(i32, i32)> =
                                ship.safely_adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS);
                            match speed_angle {
                                Some((speed, angle)) => {
                                    if speed == 0 {
                                        logger.log(&format!(
                                            "This shouldn't happen. The ship should remain to attack instead if it's that close. I think?"
                                        ));
                                    }
                                    logger.log(&format!(
                                        "  ship {} : speed: {}, angle: {}, target: {}, target ship: {}",
                                        ship.id,
                                        speed,
                                        angle,
                                        destination,
                                        enemy_ship.id
                                    ));
                                    enemy_ship.increment_committed_ships();
                                    (ship.id, Some(ship.thrust(speed, angle)))
                                }
                                _ => {
                                    logger.log(&format!(
                                        "  --- failed to find path to ship {} for ship {}",
                                        enemy_ship.id,
                                        ship.id
                                    ));
                                    (ship.id, None)
                                }
                            }
                        },

                        // execute defend move
                        &Move::DefendMove(enemy_ship, v) => {
                            if my_docked_ships.len() == 0 {
                                // if we get here, it probably means we have no docked ships and there
                                // aren't any good attack or dock targets. Probably screwed
                                (ship.id, Some(Command::Stay()))
                            } else {
                                let ship_to_defend = enemy_ship.nearest_entity(my_docked_ships.as_slice());

                                // kamikaze behavior?
                                if enemy_ship.hp - 100 > ship.hp {
                                    let destination = enemy_ship.get_position();
                                    let (speed, angle) = ship.route_to(&destination, &game_map);
                                    logger.log(&format!(
                                        "  ship {} : speed: {}, angle: {}, target: {}, defending {} from: {} via KAMIKAZE",
                                        ship.id,
                                        speed,
                                        angle,
                                        destination,
                                        ship_to_defend.id,
                                        enemy_ship.id
                                    ));
                                    enemy_ship.increment_committed_ships();
                                    (ship.id, Some(ship.thrust(speed, angle)))
                                } else {
                                    let (dx, dy) = (
                                        (ship_to_defend.get_position().0 - enemy_ship.get_position().0),
                                        (ship_to_defend.get_position().1 - enemy_ship.get_position().1),
                                    );
                                    let magnitude = f64::sqrt(dx.powi(2) + dy.powi(2));
                                    let destination = Position(
                                        (ship_to_defend.get_position().0 - (dx / magnitude)),
                                        (ship_to_defend.get_position().1 - (dy / magnitude)),
                                    );
                                    let (speed, angle) = ship.route_to(&destination, &game_map);
                                    let speed_angle: Option<(i32, i32)> =
                                        ship.adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS);
                                    match speed_angle {
                                        Some((speed, angle)) => {
                                            logger.log(&format!(
                                                "  ship {} : speed: {}, angle: {}, target: {}, defending {} from: {}",
                                                ship.id,
                                                speed,
                                                angle,
                                                destination,
                                                ship_to_defend.id,
                                                enemy_ship.id
                                            ));
                                            enemy_ship.increment_committed_ships();
                                            (ship.id, Some(ship.thrust(speed, angle)))
                                        }
                                        _ => {
                                            logger.log(&format!(
                                                "  --- failed to find path to ship {} for ship {}",
                                                enemy_ship.id,
                                                ship.id
                                            ));
                                            (ship.id, None)
                                        }
                                    }
                                }
                            }
                        }

                        // execute intercept move
                        &Move::InterceptMove(enemy_ship, v) => {
                            let destination = enemy_ship.get_position();
                            let (speed, angle) = ship.route_to(&destination, &game_map);
                            let speed_angle: Option<(i32, i32)> =
                                        // need safely_adjust variant which does not avoid enemies, only
                                        // friendlies
                                        ship.adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS);
                            match speed_angle {
                                Some((speed, angle)) => {
                                    logger.log(&format!(
                                        "  ship {} : speed: {}, angle: {}, target: {}, intercepting {}",
                                        ship.id,
                                        speed,
                                        angle,
                                        destination,
                                        enemy_ship.id
                                    ));
                                    enemy_ship.increment_committed_ships();
                                    (ship.id, Some(ship.thrust(speed, angle)))
                                }
                                _ => {
                                    logger.log(&format!(
                                        "  --- failed to find path to ship {} for ship {}",
                                        enemy_ship.id,
                                        ship.id
                                    ));
                                    (ship.id, None)
                                }
                            }
                        }
                        _ => assert_unreachable!(),
                    }
                };

                match command {
                    Some(command) => {
                        command_queue.push(command);
                        let ship: &Ship = game_map.get_ship(ship_id);
                        ship.command.set(Some(command));
                        let index = all_ship_moves
                            .iter()
                            .position(|s_m| s_m.ship.id == ship.id)
                            .unwrap();
                        all_ship_moves.remove(index);
                        if let Command::Thrust(s_id, speed, angle) = command {
                            ship.set_velocity(
                                speed as f64 * (angle as f64).to_radians().cos(),
                                speed as f64 * (angle as f64).to_radians().sin(),
                            );
                        }
                        commands_issued += 1;
                        break;
                    }
                    None => {
                        *attempted_commands.get_mut(&ship_id).unwrap() += 1;
                        if attempted_commands[&ship_id] as f64 >= (200 as f64 / ship_advantage) {
                            game_map
                                .get_ship(ship_id)
                                .command
                                .set(Some(Command::Stay()));
                            commands_issued += 1;
                            break;
                        }
                        all_ship_moves
                            .iter_mut()
                            .find(|s_m| s_m.ship.id == ship_id)
                            .unwrap()
                            .update_best_move();
                    }
                }
            } // while true
        }
        for command in command_queue.iter() {
            logger.log(&format!("{}", command.encode()));
        }
        game.send_command_queue(command_queue);
        logger.log(&format!(
            "  turn time: {}\n\n",
            start_time.to(PreciseTime::now())
        ));
    }
}
