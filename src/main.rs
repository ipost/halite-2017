/* This is a Rust implementation of the Settler starter bot for Halite-II
 * For the most part, the code is organized like the Python version, so see
 * that
 * code for more information.
 * */

mod hlt;

use hlt::entity::{DockingStatus, Entity, Planet, Ship};
use hlt::game::Game;
use hlt::logging::Logger;
use hlt::command::Command;
use hlt::constants::{MAX_CORRECTIONS, WEAPON_RADIUS};
extern crate time;
use time::PreciseTime;

fn main() {
    // Initialize the game
    let bot_name = "memetron_420";
    let game = Game::new(bot_name);
    // Initialize logging
    let mut logger = Logger::new(game.my_id);
    logger.log(&format!("Starting my {} bot!", bot_name));

    // For each turn
    let mut turn_number: usize = 0;
    loop {
        let start_time = PreciseTime::now();
        turn_number = turn_number + 1;
        // Update the game state
        let game_map = game.update_map();
        let mut command_queue: Vec<Command> = Vec::new();

        // Loop over all of our player's ships
        let ships = game_map.get_me().all_ships();
        let ship_ids = ships
            .iter()
            .map(|s| s.id.to_string())
            .collect::<Vec<String>>()
            .join(" ");
        logger.log(&format!("turn {}, my ships: {}", turn_number, ship_ids));
        let mut ships_to_order = vec![];
        for ship in ships {
            // Ignore ships that are docked or in the process of (un)docking
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

         prefer planets with at least 3 ports and farther from the enemy on turn one

        let mut planets_to_dock: Vec<&Planet> = vec![];
        for p in game_map.all_planets() {
            planets_to_dock.push(p);
        }
        let mut planets_to_dock: Vec<&Planet> = planets_to_dock
            .iter()
            .filter(|p| {
                !p.is_owned() || (p.is_owned() && p.owner.unwrap() == game.my_id as i32 && p.open_docks() > 0)
            })
            .map(|p| *p)
            .collect();

        let mut enemy_ships = game_map
            .enemy_ships()
            .iter()
            .map(|s| *s)
            .collect::<Vec<&Ship>>();

        // are ships ever getting orders after the first go-around?
        while ships_to_order.len() > 0 {
            logger.log(&format!(
                "  Ships awaiting orders: {}",
                ships_to_order.len()
            ));
            // sort ships by their distance to their nearest dockable planet
            if planets_to_dock.len() > 0 {
                ships_to_order.sort_by(|s1, s2| {
                    s1.distance_to({
                        planets_to_dock.sort_by(|p1, p2| {
                            s1.distance_to(*p1)
                                .partial_cmp(&s1.distance_to(*p2))
                                .unwrap()
                        });
                        *planets_to_dock.first().unwrap()
                    }).partial_cmp(&s2.distance_to({
                            planets_to_dock.sort_by(|p1, p2| {
                                s2.distance_to(*p1)
                                    .partial_cmp(&s2.distance_to(*p2))
                                    .unwrap()
                            });
                            *planets_to_dock.first().unwrap()
                        }))
                        .unwrap()
                });
            }

            ships_to_order.retain(|ship| {
                planets_to_dock.sort_by(|p1, p2| {
                    p1.distance_to_surface(*ship)
                        .partial_cmp(&p2.distance_to_surface(*ship))
                        .unwrap()
                });
                enemy_ships.sort_by(|s1, s2| {
                    s1.distance_to_surface(*ship)
                        .partial_cmp(&s2.distance_to_surface(*ship))
                        .unwrap()
                });
                let mut plnts_iter = planets_to_dock.iter();
                let mut ships_iter = enemy_ships.iter();
                let mut closest_planet = plnts_iter.next();
                let mut closest_e_ship = ships_iter.next();
                while closest_planet.is_some() || closest_e_ship.is_some() {
                    if !closest_e_ship.is_some()
                        || (closest_planet.is_some() && closest_e_ship.is_some()
                            && ship.distance_to_surface(*closest_planet.unwrap())
                                < ship.distance_to_surface(*closest_e_ship.unwrap()))
                    {
                        // go to planet
                        let planet = closest_planet.unwrap();

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

                        // continue if enough ships have been committed to fill all docks
                        if (planet.num_docking_spots
                            - (planet.committed_ships.get() + planet.docked_ships.len() as i32))
                            == 0
                        {
                            closest_planet = plnts_iter.next();
                            continue;
                        }
                        let destination = &ship.closest_point_to(*planet, 3.0);
                        let navigate_command: Option<Command> = ship.navigate(destination, &game_map, MAX_CORRECTIONS);
                        match navigate_command {
                            Some(command) => {
                                if let Command::Thrust(ship_id, magnitude, angle) = command {
                                    if magnitude == 0 {
                                        closest_planet = plnts_iter.next();
                                        continue;
                                    }
                                    ship.velocity_x
                                        .set(magnitude as f64 * (angle as f64).to_radians().cos());
                                    ship.velocity_y
                                        .set(magnitude as f64 * (angle as f64).to_radians().sin());
                                    logger.log(&format!(
                                        "  ship {} : speed: {}, angle: {}, target: {}, target planet: {}",
                                        ship_id,
                                        magnitude,
                                        angle,
                                        destination.as_string(),
                                        planet.id
                                    ));
                                }
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
                        match ship.navigate(destination, &game_map, MAX_CORRECTIONS) {
                            Some(command) => {
                                if let Command::Thrust(ship_id, magnitude, angle) = command {
                                    if magnitude == 0 {
                                        logger.log(&format!(
                                            "This shouldn't happen. The ship should remain to attack instead if it's that close. I think?"
                                        ));
                                        // return true;
                                    }
                                    ship.velocity_x
                                        .set(magnitude as f64 * (angle as f64).to_radians().cos());
                                    ship.velocity_y
                                        .set(magnitude as f64 * (angle as f64).to_radians().sin());
                                    logger.log(&format!(
                                        "  ship {} : speed: {}, angle: {}, target: {}, target ship: {}",
                                        ship_id,
                                        magnitude,
                                        angle,
                                        destination.as_string(),
                                        enemy_ship.id
                                    ));
                                }
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
            "  turn time: {}\n\n\n",
            start_time.to(PreciseTime::now())
        ));
    }
}
