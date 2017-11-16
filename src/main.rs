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
use hlt::game_map::GameMap;
use hlt::constants::{ATTACK_PREFERENCE_2P, ATTACK_PREFERENCE_4P, DEFEND_PREFERENCE_2P, DEFEND_PREFERENCE_4P,
                     DOCK_PREFERENCE_2P, DOCK_PREFERENCE_4P, DOCK_RADIUS, DOCK_TURNS, MAX_CORRECTIONS, MAX_SPEED,
                     WEAPON_RADIUS};
extern crate time;
use time::PreciseTime;
use std::cmp::Ordering;

macro_rules! assert_unreachable (
    () => { panic!(format!("line {}", line!())) }
    );

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
}

impl<'a> Move<'a> {
    pub fn value(&self) -> f64 {
        match self {
            &Move::DockMove(p, d) => d,
            &Move::RaidMove(s, d) => d,
            &Move::DefendMove(s, d) => d,
        }
    }

    pub fn commitment(&self) -> i32 {
        match self {
            //&Move::DockMove(p, d) => p.committed_ships.get(),
            &Move::DockMove(p, d) => 0,
            &Move::RaidMove(s, d) => s.committed_ships.get(),
            &Move::DefendMove(s, d) => s.committed_ships.get(),
        }
    }
}

#[derive(Debug)]
struct ShipMoves<'a> {
    ship: &'a Ship,
    dock_moves: Vec<Move<'a>>,
    raid_moves: Vec<Move<'a>>,
    defend_moves: Vec<Move<'a>>,
    best_move: usize,
}

impl<'a> ShipMoves<'a> {
    // moves must be sorted by value within their type
    pub fn update_best_move(&mut self) {
        match self.best_move {
            0 => self.dock_moves.remove(0),
            1 => self.raid_moves.remove(0),
            2 => self.defend_moves.remove(0),
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

            let max_ship_commitment_disparity = 1;
            let best_move = moves
                .into_iter()
                .filter(|m| m.is_some())
                .map(|m| m.unwrap())
                .min_by(|move1, move2| {
                    let commit_cmp = if (move1.commitment() - move2.commitment()).abs()
                        >= max_ship_commitment_disparity
                        {
                            move1
                                .commitment()
                                .partial_cmp(&move2.commitment())
                                .unwrap()
                        } else {
                            Ordering::Equal
                        };
                    match commit_cmp {
                        Ordering::Equal => move1.value().partial_cmp(&move2.value()).unwrap(),
                        _ => commit_cmp,
                    }
                });

            match best_move.unwrap() {
                &Move::DockMove(p, d) => 0,
                &Move::RaidMove(s, d) => 1,
                &Move::DefendMove(s, d) => 2,
            }
        };
    }

    fn sort_moves(&mut self) {
        self.dock_moves.sort_by(|dock_move1, dock_move2| {
            match dock_move1 {
                &Move::DockMove(p, d) => d,
                _ => assert_unreachable!(),
            }.partial_cmp(&match dock_move2 {
                &Move::DockMove(p, d) => d,
                _ => assert_unreachable!(),
            })
            .unwrap()
        });

        // sort by commitment and then value
        // !!! commitment sort does not work across move types because value does not
        // change
        let max_ship_commitment_disparity = 1;
        self.raid_moves.sort_by(|raid_move1, raid_move2| {
            let (ship1, v1) = match raid_move1 {
                &Move::RaidMove(s, v) => (s, v),
                _ => assert_unreachable!(),
            };
            let (ship2, v2) = match raid_move2 {
                &Move::RaidMove(s, v) => (s, v),
                _ => assert_unreachable!(),
            };
            let commit_cmp = if (ship1.committed_ships.get() - ship2.committed_ships.get()).abs()
                >= max_ship_commitment_disparity
                {
                    ship1
                        .committed_ships
                        .get()
                        .partial_cmp(&ship2.committed_ships.get())
                        .unwrap()
                } else {
                    Ordering::Equal
                };
            match commit_cmp {
                Ordering::Equal => v1.partial_cmp(&v2).unwrap(),
                _ => commit_cmp,
            }
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
            let commit_cmp = if (ship1.committed_ships.get() - ship2.committed_ships.get()).abs()
                > max_ship_commitment_disparity
                {
                    ship1
                        .committed_ships
                        .get()
                        .partial_cmp(&ship2.committed_ships.get())
                        .unwrap()
                } else {
                    Ordering::Equal
                };
            match commit_cmp {
                Ordering::Equal => v1.partial_cmp(&v2).unwrap(),
                _ => commit_cmp,
            }
        });
    }

    pub fn remaining_moves(&self) -> usize {
        self.dock_moves.len() + self.raid_moves.len() + self.defend_moves.len()
    }

    pub fn best_move(&self) -> &Move {
        match self.best_move {
            0 => self.dock_moves.first().unwrap(),
            1 => self.raid_moves.first().unwrap(),
            2 => self.defend_moves.first().unwrap(),
            _ => assert_unreachable!(),
        }
    }

    pub fn as_string(&self) -> String {
        format!("
ShipMoves {{
    ship_id: {}
    best_move: {:#?}
    }}",
    self.ship.id,
    self.best_move())
    }
}

fn main() {
    // Initialize the game
    let bot_name = "memetron_420v5";
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
        let (dock_preference, attack_preference, defend_preference) = if game_map.state.players.len() > 2 {
            (
                DOCK_PREFERENCE_4P,
                ATTACK_PREFERENCE_4P,
                DEFEND_PREFERENCE_4P,
            )
        } else {
            (
                DOCK_PREFERENCE_2P,
                ATTACK_PREFERENCE_2P,
                DEFEND_PREFERENCE_2P,
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
        let mut ships_to_order = vec![];
        // Ignore ships that are docked or in the process of (un)docking
        for ship in ships {
            if ship.docking_status == DockingStatus::UNDOCKED {
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
        let my_docked_ships: Vec<&Ship> = my_ships.into_iter().filter(|s| !s.is_undocked()).collect();

        let all_ship_moves: Vec<ShipMoves> = ships_to_order
            .into_iter()
            .map(|ship| {
                let mut dock_moves: Vec<Move> = planets_to_dock
                    .iter()
                    .map(|p| Move::DockMove(*p, dock_preference * ship.dock_value(p)))
                    .collect();
                let mut raid_moves: Vec<Move> = enemy_docked_ships
                    .iter()
                    .map(|enemy_ship| {
                        Move::RaidMove(
                            *enemy_ship,
                            attack_preference * ship.attack_value(enemy_ship),
                        )
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

                let best_move = 4;
                let mut s_m = ShipMoves {
                    ship,
                    dock_moves,
                    raid_moves,
                    defend_moves,
                    best_move,
                };
                s_m.refresh_best_move();
                s_m
            })
            .collect();

        // a ShipMove is a ship plus all of its possible moves and its best move
        let mut all_ship_moves = all_ship_moves;

        while all_ship_moves.len() > 0 {

            for s_m in all_ship_moves.iter_mut() {
                s_m.refresh_best_move();
            }

            let (ship_id, command) = {
                // find the current ship which has the best move to make
                let ship_to_move = all_ship_moves
                    .iter()
                    .filter(|s_m| s_m.remaining_moves() > 1)
                    .min_by(|s_m1, s_m2| {
                        s_m1.best_move().value()
                            .partial_cmp(&s_m2.best_move().value())
                            .unwrap()
                    });
                if ship_to_move.is_none() {
                    // all ships_to_move are out of possible moves
                    break;
                }
                let mut ship_to_move = ship_to_move.unwrap();
                let ship = ship_to_move.ship;
                match ship_to_move.best_move() {
                    &Move::DockMove(planet, d) => {
                        // if all dock spots are claimed no command
                        if (planet.num_docking_spots
                            - (planet.committed_ships.get() + planet.docked_ships.len() as i32))
                            == 0
                        {
                            (ship.id, None)

                        // if close enough to dock
                        } else if ship.in_dock_range(planet) {

                            // dock unless there's a enemy ship nearby
                            if enemy_undocked_ships.len() == 0 || ship.distance_to(ship.nearest_entity(enemy_undocked_ships.as_slice())) > (DOCK_TURNS * MAX_SPEED) as f64 { 
                                planet.committed_ships.set(planet.committed_ships.get() + 1);
                                logger.log(&format!("  Ship {} docking to {}", ship.id, planet.id));
                                (ship.id, Some(ship.dock(planet)))
                            } else {
                                (ship.id, None)
                            }

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

                    &Move::RaidMove(enemy_ship, d) => if ship.distance_to(enemy_ship) < WEAPON_RADIUS / 2.0 {
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

                    &Move::DefendMove(enemy_ship, d) => {
                        if turn_number == 100 && ship.id == 90 {
                            logger.log(&format!("{:#?}", my_docked_ships));
                        }
                        if my_docked_ships.len() == 0 {
                            // if we get here, it probably means we have no docked ships and there
                            // aren't any good attack or dock targets. Probably screwed
                            (ship.id, None)
                        } else {
                            let ship_to_defend = enemy_ship.nearest_entity(my_docked_ships.as_slice());
                            let destination = Position(
                                (ship_to_defend.get_position().0 + enemy_ship.get_position().0) / 2.0,
                                (ship_to_defend.get_position().1 + enemy_ship.get_position().1) / 2.0,
                            );
                            let (speed, angle) = ship.route_to(&destination, &game_map);
                            let speed_angle: Option<(i32, i32)> =
                                ship.safely_adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS);
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
                }
                None => {
                    // log?: was unable to do thing
                    all_ship_moves
                        .iter_mut()
                        .find(|s_m| s_m.ship.id == ship_id)
                        .unwrap()
                        .update_best_move();
                }
            }
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
