/*
 * This is a Rust implementation of the Settler starter bot for Halite-II
 * For the most part, the code is organized like the Python version, so see that
 * code for more information.
 */

mod hlt;

use hlt::entity::{Entity, DockingStatus};
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
    loop {
        logger.log("turn");
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
            for planet in game_map.all_planets() {
                // Ignore unowned planets
                if planet.is_owned() {
                    continue;
                }

                if ship.can_dock(planet) {
                    // If we are close enough to dock, do it!
                    command_queue.push(ship.dock(planet))
                } else {
                    // If not, navigate towards the planet
                    let navigate_command = ship.navigate(&ship.closest_point_to(planet, 3.0), &game_map, 90);
                    match navigate_command {
                        Some(command) => {
                            if let Command::Thrust(ship_id, magnitude, angle) = command {
                                ship.velocity_x.set(magnitude as f64 * (angle as f64).to_radians().cos());
                                ship.velocity_y.set(magnitude as f64 * (angle as f64).to_radians().sin());
                                logger.log(&format!("{} : velocity: {}, {}", ship_id, ship.velocity_x.get(), ship.velocity_y.get()));
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
