
use std::io::stdin;
use hlt::parse::Decodable;
use hlt::entity::GameState;
use hlt::command::Command;
use hlt::game_map::GameMap;
use time::PreciseTime;

#[derive(Debug)]
pub struct Game {
    pub my_id: usize,
    pub map_width: i32,
    pub map_height: i32,
}

impl Game {
    fn read_line() -> String {
        let mut buffer = String::new();
        stdin().read_line(&mut buffer).expect("Read error");
        return buffer;
    }

    fn read_id() -> usize {
        let line = Game::read_line();
        let parts = line.split_whitespace();
        let mut iter = parts.into_iter();
        return usize::parse(&mut iter);
    }

    fn read_size() -> (i32, i32) {
        let line = Game::read_line();
        let parts = line.split_whitespace();
        let mut iter = parts.into_iter();
        let width = i32::parse(&mut iter);
        let height = i32::parse(&mut iter);
        return (width, height);
    }

    pub fn new(name: &str) -> Game {
        let my_id = Game::read_id();
        let (map_width, map_height) = Game::read_size();

        println!("{}", name);

        let game = Game {
            my_id,
            map_width,
            map_height,
        };
        game.create_map();
        game
    }

    pub fn create_map(&self) -> GameMap {
        let line = Game::read_line();
        let parts = line.split_whitespace();
        let mut iter = parts.into_iter();
        let game_state = GameState::parse(&mut iter);
        return GameMap::new(self, game_state);
    }

    pub fn update_map(&self, previous_map: GameMap) -> (GameMap, PreciseTime) {
        let line = Game::read_line();
        let start_time = PreciseTime::now();
        let parts = line.split_whitespace();
        let mut iter = parts.into_iter();
        let mut game_state = GameState::parse(&mut iter);
        if previous_map.state.players.len() > 0 {
            for player in game_state.players.iter_mut() {
                let previous_ships = previous_map.state.players[player.id as usize].all_ships();
                player.strength = player.ships.len() as f64;
                for mut ship in player.ships.iter_mut() {
                    ship.owner_id = player.id;
                    let previous_ship = previous_ships.iter().find(|s| s.id == ship.id);
                    match previous_ship {
                        Some(previous_ship) => {
                            let new_pos = ship.get_positions().pop().unwrap();
                            let mut positions = previous_ship.get_positions();
                            positions.push(new_pos);
                            ship.set_positions(positions);
                        }
                        None => {
                            // ship did not exist last turn
                        }
                    }
                }
            }
        }
        return (GameMap::new(self, game_state), start_time);
    }

    pub fn send_command_queue(&self, commands: Vec<Command>) {
        for command in commands {
            print!("{}", command.encode());
        }
        println!();
    }
}
