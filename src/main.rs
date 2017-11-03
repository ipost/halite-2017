/*
 * This is a Rust implementation of the Settler starter bot for Halite-II
 * For the most part, the code is organized like the Python version, so see that
 * code for more information.
 */

mod hlt;

use hlt::entity::{Entity, DockingStatus, Planet, Ship};
use hlt::game::Game;
use hlt::logging::Logger;
use hlt::command::Command;
extern crate time;
use time::{PreciseTime};

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
        let ships: &Vec<Ship> = game_map.get_me().all_ships();
        let ship_ids = ships.iter().map(|s|
                                        s.id.to_string()
                                       ).collect::<Vec<String>>().join(" ");
        logger.log(&format!("turn {}, my ships: {}", turn_number, ship_ids));
        let mut ships_to_order = vec![];
        for ship in ships {
            ships_to_order.push(ship);
        }
        let mut remaining = ships_to_order.len();
        while ships_to_order.len() > 0 {
            logger.log(&format!("  Ships awaiting orders: {}", ships_to_order.len()));
            ships_to_order.retain(|ship|
                // Ignore ships that are docked or in the process of docking
                if ship.docking_status != DockingStatus::UNDOCKED {
                    logger.log(&format!("  ship {} will remain {}", ship.id, ship.docking_status));
                    return false;
                } else {

                    let mut planets_by_distance = game_map.all_planets().iter().collect::<Vec<&Planet>>();
                    planets_by_distance.sort_by(|p1, p2| p1.distance_to(*ship).partial_cmp(&p2.distance_to(*ship)).unwrap());
                    for planet in planets_by_distance.iter() {
                        // Skip a planet if I own it and it has no open docks
                        if planet.is_owned() && (planet.owner.unwrap() == game.my_id as i32) && planet.open_docks() == 0 {
                            continue;
                        }

                        if ship.can_dock(planet) {
                            let c = ship.dock(planet);
                            logger.log(&format!("  Ship {} docking to {}", ship.id, planet.id));
                            command_queue.push(c);
                            ship.command.set(Some(c));
                            return false
                        } else {
                            let navigate_command: Option<Command> = ship.navigate(&ship.closest_point_to(*planet, 3.0), &game_map, 60);
                            match navigate_command {
                                Some(command) => {
                                    if let Command::Thrust(ship_id, magnitude, angle) = command {
                                        ship.velocity_x.set(magnitude as f64 * (angle as f64).to_radians().cos());
                                        ship.velocity_y.set(magnitude as f64 * (angle as f64).to_radians().sin());
                                        logger.log(&format!("  ship {} : speed: {}, angle: {}", ship_id, magnitude, angle));
                                    }
                                    command_queue.push(command);
                                    ship.command.set(Some(command));
                                    return false
                                },
                                _ => {}
                            }
                        }
                        break;
                    }
                    true
                }
            );
            if ships_to_order.len() == remaining {
                logger.log(&ships_to_order.iter().map(|s|
                                                      format!("  ship {} received no command", s.id)
                                                     ).collect::<Vec<String>>().join("\n"));
                break
            } else {
                remaining = ships_to_order.len()
            }
        }
        // Send our commands to the game
        // for c in &command_queue {
        //     logger.log(&c.encode());
        // }
        game.send_command_queue(command_queue);
        logger.log(&format!("  turn time: {}", start_time.to(PreciseTime::now())));
    }
}
