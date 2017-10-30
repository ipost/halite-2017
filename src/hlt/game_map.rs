
use hlt::game::Game;
use hlt::entity::{GameState, Planet};
use hlt::player::Player;
use hlt::collision::intersect_segment_circle;
use hlt::entity::{Entity, Ship};

pub struct GameMap<'a> {
    game: &'a Game,
    state: GameState,
}

impl<'a> GameMap<'a> {
    pub fn new(game: &Game, state: GameState) -> GameMap {
        return GameMap { game, state };
    }

    pub fn all_planets(&self) -> &Vec<Planet> {
        &self.state.planets
    }

    pub fn all_ships(&self) -> Vec<&Ship> {
        self.state.players.iter().flat_map(|p| p.all_ships()).collect()
    }

    pub fn get_me(&self) -> &Player {
        let my_id = self.game.my_id;
        let player = &self.state.players[my_id];
        return player;
    }

    pub fn obstacles_between<T: Entity>(&self, ship: &Ship, target: &T) -> bool {
        for planet in self.all_planets() {
            if intersect_segment_circle(ship, target, planet, ship.get_radius() + 0.1) {
                return true;
            }
        }
        // all ships which are not undocked are also stationary obstacles
        for other_ship in self.all_ships().iter().filter(|s| !s.is_undocked()) {
            if intersect_segment_circle(ship, target, *other_ship, ship.get_radius() + 0.05) {
                return true;
            }
        }
        false
    }
}
