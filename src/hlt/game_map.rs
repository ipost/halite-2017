
use hlt::game::Game;
use hlt::entity::{Entity, GameState, Obstacle, Planet, Position, Ship};
use hlt::player::Player;
use hlt::collision::intersect_segment_circle;
use hlt::constants::SHIP_RADIUS;

pub struct GameMap<'a> {
    game: &'a Game,
    pub state: GameState,
}

impl<'a> GameMap<'a> {
    pub fn new(game: &Game, state: GameState) -> GameMap {
        return GameMap { game, state };
    }

    pub fn all_planets(&self) -> &Vec<Planet> {
        &self.state.planets
    }

    fn all_planet_obstacles(&self) -> Vec<Obstacle> {
        self.state
            .planets
            .iter()
            .map(|p| {
                if p.doomed.get() {
                    p.get_danger_obstacle()
                } else {
                    p.get_obstacle()
                }
            })
            .collect()
    }

    fn my_ship_obstacles(&self, excluded_ship: &Ship) -> Vec<Obstacle> {
        self.my_ships()
            .into_iter()
            .filter(|s| s.id != excluded_ship.id)
            .map(|s| s.get_obstacle())
            .collect::<Vec<Obstacle>>()
    }

    fn enemy_docked_ship_obstacles(&self) -> Vec<Obstacle> {
        self.enemy_ships()
            .into_iter()
            .filter(|s| !s.is_undocked())
            .map(|s| s.get_obstacle())
            .collect::<Vec<Obstacle>>()
    }

    fn enemy_undocked_ship_danger_obstacles(&self) -> Vec<Obstacle> {
        self.enemy_ships()
            .into_iter()
            .filter(|s| s.is_undocked())
            .map(|s| s.get_danger_obstacle())
            .collect::<Vec<Obstacle>>()
    }

    pub fn obstacles_for_dock(&self, docking_ship: &Ship) -> Vec<Obstacle> {
        let mut obstacles: Vec<Obstacle> = vec![];
        obstacles.append(&mut self.all_planet_obstacles());
        obstacles.append(&mut self.my_ship_obstacles(docking_ship));
        obstacles.append(&mut self.enemy_docked_ship_obstacles());
        obstacles.append(&mut self.enemy_undocked_ship_danger_obstacles());
        obstacles
    }

    pub fn obstacles_for_raid(&self, raiding_ship: &Ship) -> Vec<Obstacle> {
        let mut obstacles: Vec<Obstacle> = vec![];
        obstacles.append(&mut self.all_planet_obstacles());
        obstacles.append(&mut self.my_ship_obstacles(raiding_ship));
        obstacles.append(&mut self.enemy_docked_ship_obstacles());
        obstacles.append(&mut self.enemy_undocked_ship_danger_obstacles());
        obstacles
    }

    pub fn obstacles_for_raid_ignore_defenders(&self, raiding_ship: &Ship, target_ship: &Ship) -> Vec<Obstacle> {
        let mut obstacles: Vec<Obstacle> = vec![];
        obstacles.append(&mut self.all_planet_obstacles());
        obstacles.append(&mut self.my_ship_obstacles(raiding_ship));
        obstacles.append(&mut self.enemy_docked_ship_obstacles());
        let defender_ids: Vec<i32> = target_ship.defenders(self).iter().map(|s| s.id).collect();
        for enemy_ship in self.enemy_ships().into_iter().filter(|s| s.is_undocked()) {
            if defender_ids.contains(&enemy_ship.id) {
                obstacles.push(enemy_ship.get_obstacle())
            } else {
                obstacles.push(enemy_ship.get_danger_obstacle())
            }
        }
        obstacles
    }

    pub fn obstacles_for_raid_kamikaze(&self, raiding_ship: &Ship) -> Vec<Obstacle> {
        let mut obstacles: Vec<Obstacle> = vec![];
        obstacles.append(&mut self.all_planet_obstacles());
        obstacles.append(&mut self.my_ship_obstacles(raiding_ship));
        obstacles.append(&mut self.enemy_ships()
            .into_iter()
            .filter(|s| s.is_undocked())
            .map(|s| s.get_obstacle())
            .collect::<Vec<Obstacle>>());
        obstacles
    }

    pub fn obstacles_for_flee(&self, fleeing_ship: &Ship) -> Vec<Obstacle> {
        self.obstacles_for_dock(fleeing_ship)
    }

    pub fn obstacles_for_eradicate(&self, ship: &Ship, player_id: i32) -> Vec<Obstacle> {
        let mut obstacles: Vec<Obstacle> = vec![];
        obstacles.append(&mut self.all_planet_obstacles());
        obstacles.append(&mut self.my_ship_obstacles(ship));
        obstacles.append(&mut self.enemy_ships()
            .into_iter()
            .filter(|s| s.owner_id != player_id && !s.is_undocked())
            .map(|s| s.get_obstacle())
            .collect::<Vec<Obstacle>>());
        obstacles.append(&mut self.enemy_ships()
            .into_iter()
            .filter(|s| s.owner_id != player_id && s.is_undocked())
            .map(|s| s.get_danger_obstacle())
            .collect::<Vec<Obstacle>>());
        obstacles
    }

    pub fn obstacles_for_defend(&self, defending_ship: &Ship) -> Vec<Obstacle> {
        let mut obstacles: Vec<Obstacle> = vec![];
        obstacles.append(&mut self.all_planet_obstacles());
        obstacles.append(&mut self.my_ship_obstacles(defending_ship));
        obstacles.append(&mut self.enemy_docked_ship_obstacles());
        obstacles.append(&mut self.enemy_ships()
            .into_iter()
            .filter(|s| s.is_undocked())
        // by including this, the ship will not try to avoid combat but will try to avoid running
        // into the enemy. TODO could omit altogether to allow collisions
            .map(|s| s.get_obstacle())
            .collect::<Vec<Obstacle>>());
        obstacles
    }

    pub fn obstacles_for_intercept(&self, intercepting_ship: &Ship) -> Vec<Obstacle> {
        self.obstacles_for_defend(intercepting_ship)
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

    pub fn my_ships(&self) -> Vec<&Ship> {
        self.state
            .players
            .iter()
            .filter(|p| p.id == self.game.my_id as i32)
            .flat_map(|p| p.all_ships())
            .collect()
    }

    pub fn get_me(&self) -> &Player {
        let my_id = self.game.my_id;
        let player = &self.state.players[my_id];
        return player;
    }

    pub fn get_ship(&self, ship_id: i32) -> &Ship {
        self.all_ships().iter().find(|s| s.id == ship_id).unwrap()
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
                obstacle = Some(planet.get_obstacle());
            }
        }
        // all ships which are not undocked are also stationary obstacles
        for other_ship in self.all_ships().iter().filter(|s| !s.is_undocked()) {
            let distance_to_surface = other_ship.distance_to(start) - (SHIP_RADIUS + other_ship.get_radius() + fudge);
            if distance_to_surface < dist
                && intersect_segment_circle(start, destination, *other_ship, fudge + SHIP_RADIUS)
            {
                dist = distance_to_surface;
                obstacle = Some(other_ship.get_obstacle());
            }
        }
        obstacle
    }

    pub fn width(&self) -> f64 {
        self.game.map_width as f64
    }

    pub fn height(&self) -> f64 {
        self.game.map_height as f64
    }

    pub fn center(&self) -> Position {
        Position {
            0: self.width() / 2.0,
            1: self.height() / 2.0,
        }
    }
}
