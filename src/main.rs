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
        turn_number = turn_number + 1;
        logger.log(&format!("turn {}", turn_number));
        // Update the game state
        let game_map = game.update_map();
        let mut command_queue: Vec<Command> = Vec::new();

        // Loop over all of our player's ships
        let ships = game_map.get_me().all_ships();
        for ship in ships {
            // Ignore ships that are docked or in the process of docking
            if ship.docking_status != DockingStatus::UNDOCKED {
                continue;
            }

            // Loop over all planets
            let mut planets_by_distance = game_map.all_planets().iter().collect::<Vec<&Planet>>();
            planets_by_distance.sort_by(|p1, p2| p1.distance_to(ship).partial_cmp(&p2.distance_to(ship)).unwrap());
            for planet in planets_by_distance.iter() {
                // Skip a planet if I own it and it has no open docks
                if planet.is_owned() && (planet.owner.unwrap() == game.my_id as i32) && planet.open_docks() == 0 {
                    continue;
                }

                if ship.can_dock(planet) {
                    // If we are close enough to dock, do it!
                    let c = ship.dock(planet);
                    command_queue.push(c.clone());
                    ship.command.set(Some(c));
                } else {
                    // If not, navigate towards the planet
                    let navigate_command = ship.navigate(&ship.closest_point_to(*planet, 3.0), &game_map, 90);
                    match navigate_command {
                        Some(command) => {
                            if let Command::Thrust(ship_id, magnitude, angle) = command {
                                ship.velocity_x.set(magnitude as f64 * (angle as f64).to_radians().cos());
                                ship.velocity_y.set(magnitude as f64 * (angle as f64).to_radians().sin());
                                //logger.log(&format!("{} : velocity: {}, {}", ship_id, ship.velocity_x.get(), ship.velocity_y.get()));
                            }
                            command_queue.push(command);
                        },
                        _ => {}
                    }
                }
                break;
            }
        }
        // Send our commands to the game
        game.send_command_queue(command_queue);
    }
}
