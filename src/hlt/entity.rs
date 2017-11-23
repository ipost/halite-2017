use std::cell::Cell;
use std::cmp::{max, min};
use std::fmt;

use hlt::pathfind::{long_angle_around, short_angle_around};
use hlt::parse::Decodable;
use hlt::command::Command;
use hlt::constants::{DOCK_RADIUS, DOCK_TURNS, FUDGE, MAX_SHIP_HEALTH, MAX_SPEED, SHIP_COST, SHIP_RADIUS, WEAPON_RADIUS};
use hlt::player::Player;
use hlt::game_map::GameMap;
use hlt::logging::Logger;
extern crate time;
use time::PreciseTime;
use hlt::macros::*;
macro_rules! assert_unreachable (
    () => { panic!(format!("line {}", line!())) }
    );

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
    pub positions: Vec<Position>,
    // TODO: ^^
    pub hp: i32,
    pub velocity_x: Cell<f64>,
    pub velocity_y: Cell<f64>,
    pub docking_status: DockingStatus,
    pub docked_planet: Option<i32>,
    pub progress: i32,
    pub cooldown: i32,
    pub command: Cell<Option<Command>>,
    pub committed_ships: Cell<i32>,
    // TODO: make ^ list of ships instead of IDs
}

impl Ship {
    pub fn thrust(&self, magnitude: i32, angle: i32) -> Command {
        Command::Thrust(self.id, magnitude, angle)
    }

    pub fn dock(&self, planet: &Planet) -> Command {
        Command::Dock(self.id, planet.id)
    }

    #[allow(dead_code)]
    pub fn undock(&self) -> Command {
        Command::Undock(self.id)
    }

    pub fn increment_committed_ships(&self) {
        self.committed_ships.set(1 + self.commitment());
    }

    pub fn commitment(&self) -> i32 {
        self.committed_ships.get()
    }

    pub fn in_dock_range(&self, planet: &Planet) -> bool {
        self.distance_to_less_than(planet, (DOCK_RADIUS + planet.radius))
    }

    pub fn in_attack_range(&self, ship: &Ship) -> bool {
        // TODO: is this correct?
        self.distance_to_less_than(ship, (WEAPON_RADIUS + SHIP_RADIUS + SHIP_RADIUS))
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

    // lower is higher priority (...)
    // distance to ship * a multiplier in (0.5 - 1.0) based on hp%
    pub fn raid_value(&self, enemy_ship: &Ship) -> f64 {
        (enemy_ship.commitment() + 1) as f64 * self.distance_to_surface(enemy_ship)
            * (0.5 + (enemy_ship.hp_percent() / 2.0))
    }

    pub fn intercept_value(&self, enemy_ship: &Ship) -> f64 {
        ((enemy_ship.commitment() + 1) * 2 + 1) as f64 * self.distance_to_surface(enemy_ship)
            * (0.5 + (enemy_ship.hp_percent() / 2.0))
    }

    // given an enemy ship, get my closest docked ship to it. That distance * (0.75
    // - 1.0 depending on docked ship hp%) is considered the 'threat'. Compare
    // threat to how close self is to the docked ship to decide if it's a sensible
    // defender
    // need to figure out how to ensure this is comparable to attack_value
    pub fn defense_value(&self, enemy_ship: &Ship, game_map: &GameMap) -> f64 {
        if !game_map.get_me().owns_ship(self.id) {
            panic!(format!("defense_value() called on not my ship: {:?}", self))
        }
        if game_map.get_me().owns_ship(enemy_ship.id) {
            panic!(format!(
                "defense_value() called with my ship: {:?}",
                enemy_ship
            ))
        }

        // maybe also filter out my ships which will have undocked by the time the
        // enemy ship could arrive? Not necessary until undocking implemented
        let my_docked_ships: Vec<&Ship> = game_map
            .my_ships()
            .into_iter()
            .filter(|s| !s.is_undocked())
            .collect();

        if my_docked_ships.len() > 0 {
            let nearest_docked_ship = enemy_ship.nearest_entity(my_docked_ships.as_slice());
            let threat = enemy_ship.distance_to_surface(nearest_docked_ship)
                * (0.66667 + (nearest_docked_ship.hp_percent() / 3.0));
            // let distance_to_victim = self.distance_to_surface(nearest_docked_ship);
            // (enemy_ship.commitment() * 2 + 1) as f64 * (distance_to_victim + (threat *
            // 1.2)) // / 2.0
            let distance_to_aggressor = self.distance_to_surface(enemy_ship);
            (enemy_ship.commitment() * 9 + 1) as f64 * ((distance_to_aggressor * 0.5) + (threat * 1.5))
        } else {
            // if I have no docked ships, there's nothing to defend, unless I can attempt
            // to preemptively defend ships which are going to dock
            9999f64
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

    pub fn route_to<T: Entity>(&self, target: &T, game_map: &GameMap) -> (i32, i32) {
        let speed = MAX_SPEED;
        let nav_radius = SHIP_RADIUS + FUDGE;
        let distance = self.distance_to(target);
        let closest_stationary_obstacle: Option<Obstacle> =
            game_map.closest_stationary_obstacle(&self.get_position(), &target.get_position(), FUDGE);
        let desired_trajectory = match closest_stationary_obstacle {
            Some(obstacle) => short_angle_around(
                self.get_position(),
                target.get_position(),
                obstacle.position,
                nav_radius + obstacle.radius,
            ),
            None => self.calculate_angle_between(target),
        };
        let thrust_speed = min(speed, distance.round() as i32);
        (
            thrust_speed,
            (desired_trajectory.round() as i32 + 360) % 360,
        )
    }

    fn collide_helper(&self, other_ship: &Ship, radius: f64) -> bool {
        // TODO: optimize this? requires calculus?
        let step_count = 25;
        let mut step = 1;
        while step <= step_count {
            if self.dist_to_at_less_than(
                other_ship,
                (step as f64 / step_count as f64).clone(),
                radius,
            ) {
                return true;
            }
            step += 1;
        }
        false
    }

    pub fn will_collide_with(&self, other_ship: &Ship) -> bool {
        self.collide_helper(other_ship, (SHIP_RADIUS * 2f64) + FUDGE)
    }

    pub fn will_enter_attack_range(&self, other_ship: &Ship) -> bool {
        self.collide_helper(other_ship, (SHIP_RADIUS * 2f64) + FUDGE + WEAPON_RADIUS)
    }

    fn adjust_thrust_helper(
        &self,
        game_map: &GameMap,
        speed: i32,
        angle: i32,
        max_corrections: i32,
        avoid_enemies: bool,
    ) -> Option<(i32, i32)> {
        // let mut logger = Logger::new(0);
        let nav_radius = SHIP_RADIUS + FUDGE;
        let velocity_x = speed as f64 * (angle as f64).to_radians().cos();
        let velocity_y = speed as f64 * (angle as f64).to_radians().sin();
        let my_ships = game_map.my_ships();
        let enemy_ships: Vec<&Ship> = game_map
            .enemy_ships()
            .into_iter()
            .filter(|other| {
                other.is_undocked()

                    // too far away to possibly enter attack range
                    && self.distance_to_less_than(*other, FUDGE + WEAPON_RADIUS + (2f64 * (SHIP_RADIUS + MAX_SPEED as f64)))

                // already in attack range, can't avoid damage
                // && !self.in_attack_range(other)
            })
            .collect();

        let will_collide = |v_x, v_y| -> bool {
            // check enemy stationary ships? if not docked and more health?
            let thrust_end = Position(self.get_position().0 + v_x, self.get_position().1 + v_y);
            self.set_velocity(v_x, v_y);
            let step_count = 20;
            let collide_with_friendly_ship: bool = my_ships
                .iter()
                .filter(|other| {
                    other.id != self.id // only ships that could get close enough
                        && self.distance_to_less_than(**other, FUDGE + (2f64 * (SHIP_RADIUS + MAX_SPEED as f64)))
                        && other.is_undocked()
                })
                .any(|other| self.will_collide_with(other));
            let attacked_by_enemy = if avoid_enemies {
                enemy_ships.iter().any(|enemy| {
                    !self.in_attack_range(enemy) && self.will_enter_attack_range(enemy)
                })
            } else {
                false
            };
            self.reset_velocity();
            if collide_with_friendly_ship || attacked_by_enemy {
                return true;
            }
            game_map
                .closest_stationary_obstacle(&self.get_position(), &thrust_end, FUDGE)
                .is_some()
        };

        if !will_collide(velocity_x, velocity_y) {
            return Some((speed, (angle as i32 + 360) % 360));
        }
        let angular_step = 1;
        for i in 1..(max_corrections + 1) {
            for angular_offset in vec![i * angular_step, -1 * i * angular_step] {
                let new_angle = angle + angular_offset;
                let velocity_x = speed as f64 * (new_angle as f64).to_radians().cos();
                let velocity_y = speed as f64 * (new_angle as f64).to_radians().sin();
                if !will_collide(velocity_x, velocity_y) {
                    return Some((speed, (new_angle as i32 + 360) % 360));
                }
            }
        }
        None
    }

    pub fn safely_adjust_thrust(
        &self,
        game_map: &GameMap,
        speed: i32,
        angle: i32,
        max_corrections: i32,
    ) -> Option<(i32, i32)> {
        self.adjust_thrust_helper(game_map, speed, angle, max_corrections, true)
    }

    pub fn adjust_thrust(
        &self,
        game_map: &GameMap,
        speed: i32,
        angle: i32,
        max_corrections: i32,
    ) -> Option<(i32, i32)> {
        self.adjust_thrust_helper(game_map, speed, angle, max_corrections, false)
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
    let edge_dist_modifier = 0.50 + ((((edge_dist_x / game_map.width()) + (edge_dist_y / game_map.height()))) / 2.0);
    let size_factor = match planet.num_docking_spots {
        2 => 1.20,
        3 => 1.15,
        4 => 1.10,
        5 => 1.05,
        6 => 1.00,
        _ => assert_unreachable!(),
    };
    let planet_total = planet.commitment() + planet.docked_ships.len() as i32;
    let commitment_factor = if planet_total >= planet.num_docking_spots {
        99999f64
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
        let committed_ships = Cell::new(0);

        let ship = Ship {
            id,
            positions,
            hp,
            velocity_x,
            velocity_y,
            docking_status,
            docked_planet,
            progress,
            cooldown,
            command,
            committed_ships,
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

    pub fn increment_committed_ships(&self) {
        self.committed_ships.set(1 + self.commitment());
    }

    pub fn commitment(&self) -> i32 {
        self.committed_ships.get()
    }

    pub fn turns_until_spawn(&self) -> i32 {
        if self.docked_ships.len() == 0 {
            999999
        } else {
            (SHIP_COST - self.current_production) / (self.docked_ships.len() * 3) as i32
        }
    }

    pub fn spawn_position(&self) -> Position {
        // TODO: IMPLEMENT THIS
        Position { 0: 0f64, 1: 0f64 }
    }

    fn base_dock_value(&self, planet: &Planet, game_map: &GameMap) -> f64 {
        dock_value_helper(self, planet, game_map)
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
        Position(
            self.get_position().0 + (t * self.velocity_x.get()),
            self.get_position().1 + (t * self.velocity_y.get()),
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
        self.radius
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
