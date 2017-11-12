/* This is a Rust implementation of the Settler starter bot for Halite-II
 * For the most part, the code is organized like the Python version, so see
 * that
 * code for more information.
 * */

mod hlt;

use hlt::entity::{DockingStatus, Entity, GameState, Planet, Ship};
use hlt::game::Game;
use hlt::logging::Logger;
use hlt::command::Command;
use hlt::game_map::GameMap;
use hlt::constants::{ATTACK_PREFERENCE_2P, ATTACK_PREFERENCE_4P, DOCK_RADIUS, MAX_CORRECTIONS, MAX_SPEED,
                     WEAPON_RADIUS};
extern crate time;
use time::PreciseTime;
use std::cmp::Ordering;

struct Targets<'a> {
    ships: Vec<&'a Ship>,
    planets: Vec<&'a Planet>,
}

enum Target<'a> {
    Ship(&'a Ship),
    Planet(&'a Planet),
}

impl<'a> Targets<'a> {
    pub fn closest<T: Entity>(&self, ent: T) -> Target {
        Target::Ship(self.ships[0])
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
        let attack_preference = if game_map.state.players.len() > 2 {
            ATTACK_PREFERENCE_4P
        } else {
            ATTACK_PREFERENCE_2P
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
        let mut remaining = ships_to_order.len();

        // TODO: prefer planets with at least 3 ports and farther from the enemy on
        // turn one. Also consider how near other planets are--don't want to have
        // nothing nearby
        // for quick expansion"

        // (planet, desirability)
        // let mut planets_to_dock: Vec<(&Planet, f64)> = planets_to_dock
        let mut planets_to_dock: Vec<&Planet> = game_map
            .all_planets()
            .iter()
            .filter(|p| {
                !p.is_owned() || (p.is_owned() && p.owner.unwrap() == game.my_id as i32 && p.open_docks() > 0)
            })
            .collect();

        // (ship, desirability)
        // let mut enemy_ships: Vec<(&Ship, f64)> = game_map
        let mut enemy_ships: Vec<&Ship> = game_map.enemy_ships().iter().map(|s| *s).collect();

        let mut targets: Targets = Targets {
            ships: enemy_ships,
            planets: planets_to_dock,
        };
        // TODO: implement focus-fire (move ship into range of only one enemy ship -
        // especially docked)

        // are ships ever getting orders after the first go-around?
        while ships_to_order.len() > 0 {
            logger.log(&format!(
                "> Ships awaiting orders: {}",
                ships_to_order.len()
            ));
            // sort ships by their distance to their nearest dockable planet
            if targets.planets.len() > 0 {
                ships_to_order.sort_by(|s1, s2| {
                    s1.distance_to(s1.nearest_entity(targets.planets.as_slice()))
                        .partial_cmp(&s2.distance_to(s2.nearest_entity(targets.planets.as_slice())))
                        .unwrap()
                });
            }

            ships_to_order.retain(|ship| {
                targets.planets.sort_by(|p1, p2| {
                    p1.distance_to_surface(*ship)
                        .partial_cmp(&p2.distance_to_surface(*ship))
                        .unwrap()
                });
                // sort by number of committed ships and then by distance -- probably not
                // optimal
                // as-is navigating to enemies very far away probably doesn't make sense. Won't
                // do
                // anything until enemy_ship.committed_ships is incremented in the main loop
                targets.ships.sort_by(|s1, s2| {
                    let commit_cmp = if (s1.committed_ships.get() - s2.committed_ships.get()).abs() > 5 {
                        s1.committed_ships
                            .get()
                            .partial_cmp(&s2.committed_ships.get())
                            .unwrap()
                    } else {
                        Ordering::Equal
                    };
                    match commit_cmp {
                        Ordering::Equal => s1.distance_to_surface(*ship)
                            .partial_cmp(&s2.distance_to_surface(*ship))
                            .unwrap(),
                        _ => commit_cmp,
                    }
                });
                // enemy_ships.sort_by(|s1, s2| {
                //     (s1.distance_to_surface(*ship) * (s1.committed_ships.get() + 1) as f64)
                // .partial_cmp(&(s2.distance_to_surface(*ship) *
                // (s2.committed_ships.get() + 1) as f64))
                //             .unwrap()
                // });
                let mut plnts_iter = targets.planets.iter();
                let mut ships_iter = targets.ships.iter();
                let mut closest_planet = plnts_iter.next();
                let mut closest_e_ship = ships_iter.next();
                // TODO: maybe use distance_around_obstacle
                while closest_planet.is_some() || closest_e_ship.is_some() {
                    if !closest_e_ship.is_some()
                        || (closest_planet.is_some()
                            && (attack_preference * ship.distance_to_surface(*closest_planet.unwrap())
                                < ship.distance_to_surface(*closest_e_ship.unwrap())
                                ) // this planet is closer than the closest enemy ship
                            && (((ship.distance_to_surface(*closest_planet.unwrap()) - DOCK_RADIUS) / MAX_SPEED as f64)
                                < (closest_planet.unwrap().turns_until_spawn()) as f64
                                ) // close enough to arrive before ship spawns
                            ) {
                        // go to planet
                        let planet = closest_planet.unwrap();

                        // continue if enough ships have been committed to fill all docks
                        if (planet.num_docking_spots
                            - (planet.committed_ships.get() + planet.docked_ships.len() as i32))
                            == 0
                        {
                            closest_planet = plnts_iter.next();
                            continue;
                        }

                        // dock if possible
                        if ship.in_dock_range(planet)
                            && (!planet.is_owned()
                                || (planet.owner.unwrap() == game.my_id as i32 && planet.open_docks() > 0))
                        {
                            planet.committed_ships.set(planet.committed_ships.get() + 1);
                            let c = ship.dock(planet);
                            logger.log(&format!("  Ship {} docking to {}", ship.id, planet.id));
                            command_queue.push(c);
                            ship.command.set(Some(c));
                            return false;
                        }

                        let destination = &ship.closest_point_to(*planet, 3.0);
                        let (speed, angle) = ship.route_to(destination, &game_map);
                        let speed_angle: Option<(i32, i32)> =
                            ship.adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS);
                        match speed_angle {
                            Some((speed, angle)) => {
                                // maybe not good. sometimes ships shouldn't move to stay near their target
                                if speed == 0 {
                                    closest_planet = plnts_iter.next();
                                    continue;
                                }
                                let command = ship.thrust(speed, angle);
                                ship.velocity_x
                                    .set(speed as f64 * (angle as f64).to_radians().cos());
                                ship.velocity_y
                                    .set(speed as f64 * (angle as f64).to_radians().sin());
                                logger.log(&format!(
                                    "  ship {} : speed: {}, angle: {}, target: {}, target planet: {}",
                                    ship.id,
                                    speed,
                                    angle,
                                    destination,
                                    planet.id
                                ));
                                planet.committed_ships.set(planet.committed_ships.get() + 1);
                                command_queue.push(command);
                                ship.command.set(Some(command));
                                return false;
                            }
                            _ => {
                                logger.log(&format!(
                                    "  --- failed to find path to planet {} for ship {}",
                                    planet.id,
                                    ship.id
                                ));
                                closest_planet = plnts_iter.next();
                            }
                        }
                    } else if closest_e_ship.is_some() {
                        let enemy_ship = closest_e_ship.unwrap();
                        let destination = &ship.closest_point_to(*enemy_ship, WEAPON_RADIUS / 2.0);
                        if ship.distance_to(*enemy_ship) < WEAPON_RADIUS / 2.0 {
                            logger.log(&format!(
                                "  ship {} will remain {} to attack {}",
                                ship.id,
                                ship.docking_status,
                                enemy_ship.id
                            ));
                            ship.command.set(Some(Command::Stay()));
                            return false;
                        }
                        let (speed, angle) = ship.route_to(destination, &game_map);
                        let speed_angle: Option<(i32, i32)> =
                            ship.adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS);
                        match speed_angle {
                            Some((speed, angle)) => {
                                let command = ship.thrust(speed, angle);
                                if speed == 0 {
                                    logger.log(&format!(
                                        "This shouldn't happen. The ship should remain to attack instead if it's that close. I think?"
                                    ));
                                    // return true;
                                }
                                ship.velocity_x
                                    .set(speed as f64 * (angle as f64).to_radians().cos());
                                ship.velocity_y
                                    .set(speed as f64 * (angle as f64).to_radians().sin());
                                logger.log(&format!(
                                    "  ship {} : speed: {}, angle: {}, target: {}, target ship: {}",
                                    ship.id,
                                    speed,
                                    angle,
                                    destination,
                                    enemy_ship.id
                                ));
                                enemy_ship
                                    .committed_ships
                                    .set(1 + enemy_ship.committed_ships.get());
                                command_queue.push(command);
                                ship.command.set(Some(command));
                                return false;
                            }
                            _ => {
                                logger.log(&format!(
                                    "  --- failed to find path to ship {} for ship {}",
                                    enemy_ship.id,
                                    ship.id
                                ));
                                closest_e_ship = ships_iter.next();
                            }
                        }
                    }
                }
                true
            });

            if ships_to_order.len() == remaining {
                logger.log(&ships_to_order
                    .iter()
                    .map(|s| format!("  ship {} received no command", s.id))
                    .collect::<Vec<String>>()
                    .join("\n"));
                break;
            } else {
                remaining = ships_to_order.len()
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
