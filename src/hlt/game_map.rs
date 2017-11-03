
use hlt::game::Game;
use hlt::entity::{GameState, Planet, Position};
use hlt::player::Player;
use hlt::collision::intersect_segment_circle;
use hlt::entity::{Entity, Obstacle, Ship};
// use hlt::logging::Logger;
use hlt::constants::SHIP_RADIUS;

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
        self.state
            .players
            .iter()
            .flat_map(|p| p.all_ships())
            .collect()
    }

    pub fn enemy_ships(&self) -> Vec<&Ship> {
        self.state
            .players
            .iter()
            .filter(|p| p.id != self.game.my_id as i32)
            .flat_map(|p| p.all_ships())
            .collect()
    }

    pub fn get_me(&self) -> &Player {
        let my_id = self.game.my_id;
        let player = &self.state.players[my_id];
        return player;
    }

    pub fn closest_stationary_obstacle(
        &self,
        start: &Position,
        destination: &Position,
        fudge: f64,
    ) -> Option<Obstacle> {
        // let mut logger = Logger::new(0);
        let mut dist: f64 = 99999999f64;
        let mut obstacle: Option<Obstacle> = None;
        for planet in self.all_planets() {
            let distance_to_surface = planet.distance_to(start) - (SHIP_RADIUS + planet.get_radius() + fudge);
            if distance_to_surface < dist && intersect_segment_circle(start, destination, planet, fudge + SHIP_RADIUS) {
                dist = distance_to_surface;
                obstacle = Some(Obstacle {
                    radius: planet.get_radius(),
                    position: planet.get_position(),
                });
            }
        }
        // all ships which are not undocked are also stationary obstacles
        for other_ship in self.all_ships().iter().filter(|s| !s.is_undocked()) {
            let distance_to_surface = other_ship.distance_to(start) - (SHIP_RADIUS + other_ship.get_radius() + fudge);
            if distance_to_surface < dist
                && intersect_segment_circle(start, destination, *other_ship, fudge + SHIP_RADIUS)
            {
                dist = distance_to_surface;
                obstacle = Some(Obstacle {
                    radius: other_ship.get_radius(),
                    position: other_ship.get_position(),
                });
            }
        }
        obstacle
    }
}
