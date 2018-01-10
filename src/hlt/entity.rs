use std::cell::Cell;
use std::cmp::min;
use std::fmt;

use hlt::pathfind::short_angle_around;
use hlt::parse::Decodable;
use hlt::command::Command;
use hlt::constants::{DOCK_RADIUS, DOCK_TURNS, FUDGE, MAX_EXPLOSION_DAMAGE, MAX_SHIP_HEALTH, MAX_SPEED,
                     MIN_EXPLOSION_DAMAGE, SHIP_COST, SHIP_RADIUS, WEAPON_RADIUS};
use hlt::player::Player;
use std::collections::HashMap;
use hlt::game_map::GameMap;

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Position(pub f64, pub f64);
impl Position {}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

impl Decodable for Position {
    fn parse<'a, I>(tokens: &mut I) -> Position
    where
        I: Iterator<Item = &'a str>,
    {
        let x = f64::parse(tokens);
        let y = f64::parse(tokens);
        return Position(x, y);
    }
}

#[derive(Debug)]
pub struct Obstacle {
    pub position: Position,
    pub radius: f64,
    pub velocity_x: f64,
    pub velocity_y: f64,
}

#[derive(PartialEq, Debug)]
pub enum DockingStatus {
    UNDOCKED = 0,
    DOCKING = 1,
    DOCKED = 2,
    UNDOCKING = 3,
}

impl fmt::Display for DockingStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let printable = match *self {
            DockingStatus::UNDOCKED => "UNDOCKED",
            DockingStatus::DOCKING => "DOCKING",
            DockingStatus::DOCKED => "DOCKED",
            DockingStatus::UNDOCKING => "UNDOCKING",
        };
        write!(f, "{}", printable)
    }
}

impl Decodable for DockingStatus {
    fn parse<'a, I>(tokens: &mut I) -> DockingStatus
    where
        I: Iterator<Item = &'a str>,
    {
        let i = i32::parse(tokens);
        return match i {
            0 => DockingStatus::UNDOCKED,
            1 => DockingStatus::DOCKING,
            2 => DockingStatus::DOCKED,
            3 => DockingStatus::UNDOCKING,
            _ => panic!(format!("Not a valid docking status: {:?}", i)),
        };
    }
}

#[derive(Debug)]
pub struct Ship {
    pub id: i32,
    pub owner_id: i32,
    pub positions: Vec<Position>,
    pub hp: i32,
    pub velocity_x: Cell<f64>,
    pub velocity_y: Cell<f64>,
    pub docking_status: DockingStatus,
    pub docked_planet: Option<i32>,
    pub progress: i32,
    pub cooldown: i32,
    pub command: Cell<Option<Command>>,
}

impl Ship {
    pub fn thrust(&self, magnitude: i32, angle: i32) -> Command {
        Command::Thrust(self.id, magnitude, angle)
    }

    pub fn dock(&self, planet: &Planet) -> Command {
        Command::Dock(self.id, planet.id)
    }

    pub fn undock(&self) -> Command {
        Command::Undock(self.id)
    }

    pub fn get_obstacle(&self) -> Obstacle {
        Obstacle {
            position: self.get_position(),
            radius: SHIP_RADIUS,
            velocity_x: self.velocity_x.get(),
            velocity_y: self.velocity_y.get(),
        }
    }

    pub fn get_danger_obstacle(&self) -> Obstacle {
        Obstacle {
            position: self.get_position(),
            radius: WEAPON_RADIUS,
            velocity_x: self.velocity_x.get(),
            velocity_y: self.velocity_y.get(),
        }
    }

    pub fn in_dock_range(&self, planet: &Planet) -> bool {
        self.distance_to_less_than(planet, (DOCK_RADIUS + planet.get_radius()))
    }

    pub fn in_attack_range(&self, ship: &Ship) -> bool {
        // TODO: is this correct?
        self.distance_to_less_than(ship, (WEAPON_RADIUS + SHIP_RADIUS + SHIP_RADIUS))
    }

    pub fn enemies_in_attack_range(&self, game_map: &GameMap) -> usize {
        game_map
            .all_ships()
            .iter()
            .filter(|s| s.owner_id != self.owner_id && s.in_attack_range(self))
            .count()
    }

    pub fn is_docked(&self) -> bool {
        self.docking_status == DockingStatus::DOCKED
    }

    pub fn is_undocked(&self) -> bool {
        self.docking_status == DockingStatus::UNDOCKED
    }

    pub fn hp_percent(&self) -> f64 {
        self.hp as f64 / MAX_SHIP_HEALTH as f64
    }

    fn base_dock_value(&self, planet: &Planet, game_map: &GameMap) -> f64 {
        dock_value_helper(self, planet, game_map)
    }

    pub fn dock_value(&self, planet: &Planet, game_map: &GameMap) -> f64 {
        self.base_dock_value(planet, game_map)
            + (0.5
                * game_map
                    .all_planets()
                    .iter()
                    .filter(|p| p.id != planet.id)
                    .map(|p| planet.base_dock_value(p, game_map))
                    .filter(|v| v < &1000.0)
                    .fold(0.0, |acc, s| acc + s) / game_map.all_planets().len() as f64)
    }

    pub fn raid_value(&self, enemy_ship: &Ship, game_map: &GameMap, commitment_map: &HashMap<i32, Vec<i32>>) -> f64 {
        let defense_factor = 1.00 + (0.10 * total_ship_strength(enemy_ship.defenders(game_map).as_slice()));
        (0.2 * commitment(enemy_ship, commitment_map) + 1.0) * self.distance_to_surface(enemy_ship)
            * (0.5 + (enemy_ship.hp_percent() / 2.0)) * defense_factor
    }

    pub fn intercept_value(&self, enemy_ship: &Ship, commitment_map: &HashMap<i32, Vec<i32>>) -> f64 {
        (1.0 * commitment(enemy_ship, commitment_map) + 1.0) * self.distance_to_surface(enemy_ship)
            * scaled_to(0.75, enemy_ship.hp_percent())
    }

    pub fn defense_value(&self, enemy_ship: &Ship, game_map: &GameMap, commitment_map: &HashMap<i32, Vec<i32>>) -> f64 {
        let my_docked_ships: Vec<&Ship> = game_map
            .my_ships()
            .into_iter()
            .filter(|s| !s.is_undocked())
            .collect();

        if my_docked_ships.len() > 0 {
            let nearest_docked_ship = enemy_ship.nearest_entity(my_docked_ships.as_slice());
            let threat = enemy_ship.distance_to_surface(nearest_docked_ship)
                * scaled_to(0.66667, nearest_docked_ship.hp_percent());
            // let distance_to_victim = self.distance_to_surface(nearest_docked_ship);
            // (enemy_ship.commitment() * 2 + 1) as f64 * (distance_to_victim + (threat *
            // 1.2)) // / 2.0
            let distance_to_aggressor = self.distance_to_surface(enemy_ship);
            (1.0 * commitment(enemy_ship, commitment_map) + 1.0) * ((distance_to_aggressor * 0.5) + (threat * 1.5))
        } else {
            // if I have no docked ships, there's nothing to defend, unless I can attempt
            // to preemptively defend ships which are going to dock
            9999.0
        }
    }

    pub fn commanded(&self) -> bool {
        self.command.get().is_some()
    }

    pub fn get_positions(&self) -> Vec<Position> {
        self.positions.clone()
    }

    pub fn set_positions(&mut self, positions: Vec<Position>) {
        self.positions = positions
    }

    pub fn set_velocity(&self, v_x: f64, v_y: f64) {
        self.velocity_x.set(v_x);
        self.velocity_y.set(v_y);
    }

    pub fn reset_velocity(&self) {
        self.velocity_x.set(0f64);
        self.velocity_y.set(0f64);
    }

    pub fn defenders<'a>(&self, game_map: &'a GameMap) -> Vec<&'a Ship> {
        game_map
            .all_ships()
            .into_iter()
            .filter(|s| {
                s.owner_id == self.owner_id && s.is_undocked() && s.distance_to_less_than(self, MAX_SPEED as f64)
            })
            .collect()
    }

    pub fn projected_damage_taken(&self, game_map: &GameMap) -> i32 {
        game_map
            .all_ships()
            .into_iter()
            .filter(|s| s.is_undocked() && s.owner_id != self.owner_id)
            .map(|enemy| {
                if enemy.in_attack_range(self) {
                    64 / enemy.enemies_in_attack_range(game_map)
                } else if enemy.will_enter_attack_range(self) {
                    64
                } else {
                    0
                }
            })
            .fold(0, |acc, s| acc + s as i32)
    }

    pub fn route_to<T: Entity>(&self, target: &T, game_map: &GameMap) -> (i32, i32) {
        let speed = MAX_SPEED;
        let nav_radius = SHIP_RADIUS + FUDGE;
        let distance = self.distance_to(target);
        let closest_stationary_obstacle: Option<Obstacle> =
            game_map.closest_stationary_obstacle(&self.get_position(), &target.get_position(), FUDGE);
        let desired_trajectory = match closest_stationary_obstacle {
            Some(obstacle) => {
                // the ship is already inside the obstacle. Should only happen when the
                // obstacle is
                // a planet which will explode. In which case, fly directly away
                if self.distance_to(&obstacle.position) < obstacle.radius {
                    obstacle.position.calculate_angle_between(self)
                } else {
                    short_angle_around(
                        self.get_position(),
                        target.get_position(),
                        obstacle.position,
                        nav_radius + obstacle.radius,
                    )
                }
            }
            None => self.calculate_angle_between(target),
        };
        let thrust_speed = min(speed, distance.round() as i32);
        (
            thrust_speed,
            (desired_trajectory.round() as i32 + 360) % 360,
        )
    }

    pub fn will_enter_attack_range(&self, other_ship: &Ship) -> bool {
        check_collision(&self.get_obstacle(), &other_ship.get_danger_obstacle())
    }

    pub fn will_collide_with_obstacle(&self, obstacle: &Obstacle) -> bool {
        check_collision(&self.get_obstacle(), obstacle)
    }

    // create function which will navigate avoiding only specified entities
    // could be used to crash into planets while avoiding friendly ships, crash
    // into enemies ships, etc
    pub fn smart_navigate(
        &self,
        destination: &Position,
        game_map: &GameMap,
        obstacles: Vec<Obstacle>,
    ) -> Option<(i32, i32)> {
        // let mut logger = Logger::new(0);
        // first adjust destination to route around planets
        let closest_stationary_obstacle: Option<Obstacle> =
            game_map.closest_stationary_obstacle(&self.get_position(), destination, FUDGE);
        let desired_trajectory = match closest_stationary_obstacle {
            Some(obstacle) => {
                // the ship is already inside the obstacle. Should only happen when the
                // obstacle is
                // a planet which will explode. In which case, fly directly away
                if self.distance_to(&obstacle.position) < obstacle.radius {
                    panic!("inside something");
                    obstacle.position.calculate_angle_between(self)
                } else {
                    short_angle_around(
                        self.get_position(),
                        *destination,
                        obstacle.position,
                        SHIP_RADIUS + FUDGE + obstacle.radius,
                    )
                }
            }
            None => self.calculate_angle_between(destination),
        };
        let thrust_speed = min(MAX_SPEED, self.distance_to(destination).round() as i32);
        let desired_trajectory = (desired_trajectory.round() as i32 + 360) % 360;
        let nav_radius = SHIP_RADIUS + FUDGE;
        let velocity_x = thrust_speed as f64 * (desired_trajectory as f64).to_radians().cos();
        let velocity_y = thrust_speed as f64 * (desired_trajectory as f64).to_radians().sin();
        let destination = {
            let pos = self.get_position();
            Position {
                0: pos.0 + velocity_x,
                1: pos.1 + velocity_y,
            }
        };

        let will_collide = |v_x, v_y| -> bool {
            self.set_velocity(v_x, v_y);
            let thrust_end = self.get_position_at(1.0);

            // check for hitting walls
            if thrust_end.0 < nav_radius || thrust_end.1 < nav_radius || (game_map.width() - thrust_end.0) < nav_radius
                || (game_map.height() - thrust_end.1) < nav_radius
            {
                self.reset_velocity();
                return true;
            }

            if obstacles
                .iter()
                .any(|ob| self.will_collide_with_obstacle(ob))
            {
                // let o = obstacles.iter().find(|ob| self.will_collide_with_obstacle(ob));
                // let mut loggers = Logger::new(0);
                // loggers.log(&format!("lmao {:#?}", o));
                self.reset_velocity();
                return true;
            }

            self.reset_velocity();
            false
        };

        if !will_collide(velocity_x, velocity_y) {
            return Some((thrust_speed, (desired_trajectory as i32 + 360) % 360));
        }

        // sort these by how close they'd leave the ship to the target
        // try all speed, angle combos
        let mut possible_thrusts: Vec<(i32, i32, Position)> = Vec::with_capacity(1 + (360 * MAX_SPEED) as usize);
        possible_thrusts.push((0, 0, self.get_position()));
        for angle in 0..359 {
            for speed in 1..(MAX_SPEED + 1) {
                let v_x = speed as f64 * (angle as f64).to_radians().cos();
                let v_y = speed as f64 * (angle as f64).to_radians().sin();
                let thrust_end = Position(self.get_position().0 + v_x, self.get_position().1 + v_y);
                possible_thrusts.push((speed, angle, thrust_end));
            }
        }
        possible_thrusts.sort_by(|&(_speed1, _angle1, pos1), &(_speed2, _angle2, pos2)| {
            ((destination.0 - pos1.0).powi(2) + (destination.1 - pos1.1).powi(2))
                .partial_cmp(&((destination.0 - pos2.0).powi(2) + (destination.1 - pos2.1).powi(2)))
                .unwrap()
        });

        for (speed, angle, _end_position) in possible_thrusts {
            let velocity_x = speed as f64 * (angle as f64).to_radians().cos();
            let velocity_y = speed as f64 * (angle as f64).to_radians().sin();
            if !will_collide(velocity_x, velocity_y) {
                return Some((speed, (angle as i32 + 360) % 360));
            }
        }
        None
    }
}

pub fn total_ship_strength(ships: &[&Ship]) -> f64 {
    total_strength(&ships.iter().map(|s| s.hp).collect::<Vec<i32>>().as_slice())
}

pub fn total_strength(hps: &[i32]) -> f64 {
    let per_ship_multiplier = 0.15;
    hps.iter()
        .map(|hp| *hp as f64 / MAX_SHIP_HEALTH as f64)
        .fold(0.0, |acc, s| acc + s) * ((1.0 - per_ship_multiplier) + (per_ship_multiplier * hps.len() as f64))
}

pub fn commitment(ship: &Ship, commitment_map: &HashMap<i32, Vec<i32>>) -> f64 {
    total_strength(commitment_map.get(&ship.id).unwrap().as_slice())
}

// takes a percent x and moves it into the scale of (scale - 1.0)
// scaled_to(0.75, 0.6) == 0.9
// scaled_to(0.25, 0.2) == 0.4
fn scaled_to(scale: f64, x: f64) -> f64 {
    (x * (1.0 - scale)) + scale
}

fn check_collision(obstacle_1: &Obstacle, obstacle_2: &Obstacle) -> bool {
    let ship_vx = obstacle_1.velocity_x;
    let ob_vx = obstacle_2.velocity_x;
    let ship_vy = obstacle_1.velocity_y;
    let ob_vy = obstacle_2.velocity_y;

    let radius = obstacle_1.radius + obstacle_2.radius + FUDGE;

    let Position(ship_px, ship_py) = obstacle_1.position;
    let Position(ob_px, ob_py) = obstacle_2.position;

    let a = (ship_vx - ob_vx).powi(2) + (ship_vy - ob_vy).powi(2);
    let b = 2.0 * (ship_px - ob_px) * (ship_vx - ob_vx) + 2.0 * (ship_py - ob_py) * (ship_vy - ob_vy);
    let c = (ship_px - ob_px).powi(2) + (ship_py - ob_py).powi(2) - radius.powi(2);

    let discriminant = b.powi(2) - (4.0 * a * c);

    if discriminant < 0.0 {
        false
    } else {
        let t1 = ((-1.0 * b) - discriminant.sqrt()) / (2.0 * a);
        if 0.0 <= t1 && t1 <= 1.0 {
            return true;
        }
        let t2 = ((-1.0 * b) + discriminant.sqrt()) / (2.0 * a);
        (0.0 <= t2 && t2 <= 1.0)
    }
}

fn dock_value_helper<T: Entity>(entity: &T, planet: &Planet, game_map: &GameMap) -> f64 {
    let planet_pos = planet.get_position();
    let edge_dist_x = if planet_pos.0 > game_map.width() / 2.0 {
        game_map.width() - planet_pos.0
    } else {
        planet_pos.0
    };
    let edge_dist_y = if planet_pos.1 > game_map.height() / 2.0 {
        game_map.height() - planet_pos.1
    } else {
        planet_pos.1
    };
    let edge_dist_modifier = scaled_to(
        0.50,
        ((((edge_dist_x / game_map.width()) + (edge_dist_y / game_map.height())))),
    );

    let size_factor = match planet.num_docking_spots {
        2 => 1.30,
        3 => 1.10,
        4 => 1.10,
        5 => 1.05,
        6 => 1.00,
        _ => assert_unreachable!(),
    };
    let planet_total = planet.commitment() + planet.docked_ships.len() as i32;
    let commitment_factor = if planet_total >= planet.num_docking_spots {
        9999999f64
    } else if planet_total > 0 {
        0.85
    } else {
        1.0
    };
    // factor in if ship will spawn before I can arrive?
    commitment_factor * size_factor
        * (entity.distance_to_surface(planet)
           // because docking will put the ship out of commision for that long. I guess?
           + (2 * MAX_SPEED * (DOCK_TURNS + 0)) as f64) * edge_dist_modifier
}

impl PartialEq for Ship {
    fn eq(&self, other: &Ship) -> bool {
        self.id == other.id
    }
}

impl Decodable for Ship {
    fn parse<'a, I>(tokens: &mut I) -> Ship
    where
        I: Iterator<Item = &'a str>,
    {
        let id = i32::parse(tokens);
        let owner_id = 0;
        let positions = vec![Position::parse(tokens)];
        let hp = i32::parse(tokens);
        let velocity_x = Cell::new(f64::parse(tokens));
        let velocity_y = Cell::new(f64::parse(tokens));
        let docking_status = DockingStatus::parse(tokens);
        let docked_planet_raw = i32::parse(tokens);
        let docked_planet = match docking_status {
            DockingStatus::UNDOCKED => None,
            _ => Some(docked_planet_raw),
        };
        let progress = i32::parse(tokens);
        let cooldown = i32::parse(tokens);
        let command = Cell::new(None);

        let ship = Ship {
            id,
            owner_id,
            positions,
            hp,
            velocity_x,
            velocity_y,
            docking_status,
            docked_planet,
            progress,
            cooldown,
            command,
        };
        return ship;
    }
}

#[derive(PartialEq, Debug)]
pub struct Planet {
    pub id: i32,
    pub position: Position,
    pub hp: i32,
    pub radius: f64,
    pub num_docking_spots: i32,
    pub current_production: i32,
    pub remaining_resources: i32,
    pub owner: Option<i32>,
    pub docked_ships: Vec<i32>,
    pub committed_ships: Cell<i32>,
    pub doomed: Cell<bool>,
}

impl Planet {
    pub fn is_owned(&self) -> bool {
        self.owner.is_some()
    }

    pub fn open_docks(&self) -> usize {
        self.num_docking_spots as usize - self.docked_ships.len()
    }

    #[allow(dead_code)]
    pub fn any_docked(&self) -> bool {
        self.docked_ships.len() > 0
    }

    pub fn get_obstacle(&self) -> Obstacle {
        Obstacle {
            position: self.get_position(),
            radius: self.radius,
            velocity_x: 0.0,
            velocity_y: 0.0,
        }
    }

    pub fn increment_committed_ships(&self) {
        self.committed_ships.set(1 + self.commitment());
    }

    pub fn commitment(&self) -> i32 {
        self.committed_ships.get()
    }

    pub fn get_danger_obstacle(&self) -> Obstacle {
        Obstacle {
            position: self.get_position(),
            radius: self.explosion_radius(),
            velocity_x: 0.0,
            velocity_y: 0.0,
        }
    }

    pub fn turns_until_spawn(&self) -> i32 {
        if self.docked_ships.len() == 0 {
            999999
        } else {
            (SHIP_COST - self.current_production) / (self.docked_ships.len() * 3) as i32
        }
    }

    fn base_dock_value(&self, planet: &Planet, game_map: &GameMap) -> f64 {
        dock_value_helper(self, planet, game_map)
    }

    fn explosion_radius(&self) -> f64 {
        if self.radius <= DOCK_RADIUS {
            DOCK_RADIUS
        } else {
            self.radius
        }
    }

    #[allow(dead_code)]
    pub fn damage_from_explosion(&self, ship: &Ship) -> i32 {
        let danger_radius = self.explosion_radius();
        let distance_to_surface = self.distance_to(ship) - self.radius;
        if distance_to_surface < danger_radius {
            let explosion_damage = MIN_EXPLOSION_DAMAGE
                + ((1.0 - (distance_to_surface / danger_radius)) * (MAX_EXPLOSION_DAMAGE - MIN_EXPLOSION_DAMAGE) as f64)
                    as i32;
            if explosion_damage > ship.hp {
                ship.hp
            } else {
                explosion_damage
            }
        } else {
            0
        }
    }

    pub fn is_doomed(&self) -> bool {
        self.doomed.get()
    }
}

impl Decodable for Planet {
    fn parse<'a, I>(tokens: &mut I) -> Planet
    where
        I: Iterator<Item = &'a str>,
    {
        let id = i32::parse(tokens);
        let position = Position::parse(tokens);
        let hp = i32::parse(tokens);
        let radius = f64::parse(tokens);
        let num_docking_spots = i32::parse(tokens);
        let current_production = i32::parse(tokens);
        let remaining_resources = i32::parse(tokens);
        let owner = Option::parse(tokens);
        let docked_ships = Vec::parse(tokens);
        let committed_ships = Cell::new(0);
        let doomed = Cell::new(false);

        return Planet {
            id,
            position,
            hp,
            radius,
            num_docking_spots,
            current_production,
            remaining_resources,
            owner,
            docked_ships,
            committed_ships,
            doomed,
        };
    }
}

#[derive(PartialEq, Debug)]
pub struct GameState {
    pub players: Vec<Player>,
    pub planets: Vec<Planet>,
}

impl Decodable for GameState {
    fn parse<'a, I>(tokens: &mut I) -> GameState
    where
        I: Iterator<Item = &'a str>,
    {
        let players = Vec::parse(tokens);
        let planets = Vec::parse(tokens);

        return GameState { players, planets };
    }
}

pub trait Entity: Sized {
    fn get_position(&self) -> Position;
    fn get_position_at(&self, t: f64) -> Position;
    fn get_radius(&self) -> f64;

    fn smart_distance_to<T: Entity>(&self, target: &T) -> f64 {
        // TODO: use pathfinding algo
        let Position(x1, y1) = self.get_position();
        let Position(x2, y2) = target.get_position();
        (x2 - x1).powi(2) + (y2 - y1).powi(2)
    }

    fn distance_to_unsq<T: Entity>(&self, target: &T) -> f64 {
        let Position(x1, y1) = self.get_position();
        let Position(x2, y2) = target.get_position();
        (x2 - x1).powi(2) + (y2 - y1).powi(2)
    }

    fn distance_to<T: Entity>(&self, target: &T) -> f64 {
        f64::sqrt(self.distance_to_unsq(target))
    }

    fn distance_to_less_than<T: Entity>(&self, target: &T, query: f64) -> bool {
        self.distance_to_unsq(target) < query.powi(2)
    }

    fn distance_to_surface<T: Entity>(&self, target: &T) -> f64 {
        self.distance_to(target) - (self.get_radius() + target.get_radius())
    }

    fn dist_to_at<T: Entity>(&self, target: &T, t: f64) -> f64 {
        let Position(x1, y1) = self.get_position_at(t);
        let Position(x2, y2) = target.get_position_at(t);
        f64::sqrt((x2 - x1).powi(2) + (y2 - y1).powi(2))
    }

    fn dist_to_at_less_than<T: Entity>(&self, target: &T, t: f64, query: f64) -> bool {
        let Position(x1, y1) = self.get_position_at(t);
        let Position(x2, y2) = target.get_position_at(t);
        (x2 - x1).powi(2) + (y2 - y1).powi(2) < query.powi(2)
    }

    fn calculate_angle_between<T: Entity>(&self, target: &T) -> f64 {
        let Position(x1, y1) = self.get_position();
        let Position(x2, y2) = target.get_position();
        (f64::atan2(y2 - y1, x2 - x1).to_degrees() + 360.0) % 360.0
    }

    fn closest_point_to<T: Entity>(&self, target: &T, min_distance: f64) -> Position {
        let angle = target.calculate_angle_between(self);
        let radius = target.get_radius() + min_distance;
        let Position(target_x, target_y) = target.get_position();
        let x = target_x + radius * f64::cos(angle.to_radians());
        let y = target_y + radius * f64::sin(angle.to_radians());

        Position(x, y)
    }

    fn nearest_entity<'a, T: 'a + Entity>(&self, entities: &'a [&T]) -> &'a T {
        entities
            .iter()
            .min_by(|e1, e2| {
                if e1.get_radius() == e2.get_radius() {
                    e1.distance_to_unsq(self)
                        .partial_cmp(&e2.distance_to_unsq(self))
                } else {
                    (e1.distance_to(self) - e1.get_radius()).partial_cmp(&(e2.distance_to(self) - e2.get_radius()))
                }.unwrap()
            })
            .unwrap()
    }
}

impl Entity for Ship {
    fn get_position(&self) -> Position {
        *self.positions.last().unwrap()
    }

    fn get_position_at(&self, t: f64) -> Position {
        let pos = self.get_position();
        Position(
            pos.0 + (t * self.velocity_x.get()),
            pos.1 + (t * self.velocity_y.get()),
        )
    }

    fn get_radius(&self) -> f64 {
        SHIP_RADIUS
    }
}

impl Entity for Planet {
    fn get_position(&self) -> Position {
        self.position
    }

    fn get_position_at(&self, _t: f64) -> Position {
        self.position
    }

    fn get_radius(&self) -> f64 {
        if self.is_doomed() {
            self.radius + self.explosion_radius()
        } else {
            self.radius
        }
    }
}

impl Entity for Position {
    fn get_position(&self) -> Position {
        *self
    }

    fn get_position_at(&self, _t: f64) -> Position {
        *self
    }

    fn get_radius(&self) -> f64 {
        0.0
    }
}
