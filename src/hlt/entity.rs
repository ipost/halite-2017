

use std::cell::Cell;
use std::cmp::min;
use std::fmt;
use std::hash::{Hash, Hasher};

use hlt::pathfind::avoid;
use hlt::parse::Decodable;
use hlt::command::Command;
use hlt::constants::{DOCK_RADIUS, SHIP_RADIUS, MAX_SPEED};
use hlt::player::Player;
use hlt::game_map::GameMap;
use hlt::logging::Logger;

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Position(pub f64, pub f64);
impl Position {
    pub fn as_string(&self) -> String {
        format!("{}, {}", self.0, self.1)
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
    pub radius: f64
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
    pub position: Position,
    pub hp: i32,
    pub velocity_x: Cell<f64>,
    pub velocity_y: Cell<f64>,
    pub docking_status: DockingStatus,
    pub docked_planet: Option<i32>,
    pub progress: i32,
    pub cooldown: i32,
    pub command: Cell<Option<Command>>
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

    pub fn can_dock(&self, planet: &Planet) -> bool {
        self.distance_to(planet) <= (DOCK_RADIUS + planet.radius)
    }

    pub fn is_undocked(&self) -> bool {
        self.docking_status == DockingStatus::UNDOCKED
    }

    pub fn navigate<T: Entity>(&self, target: &T, game_map: &GameMap, max_corrections: i32) -> Option<Command> {
        let mut logger = Logger::new(0);
        let speed = MAX_SPEED;
        let fudge = 0.05;
        let effective_planet_radius_modifier = 0.6;//SHIP_RADIUS + SHIP_RADIUS + DOCK_RADIUS; // path around ship docking zone
        let distance = self.distance_to(target);
        let closest_stationary_obstacle: Option<Obstacle> = game_map.closest_stationary_obstacle(&self.get_position(), &target.get_position(), effective_planet_radius_modifier);
        let angle = match closest_stationary_obstacle {
            Some(obstacle) => {
                //logger.log(&format!("  ship {} routing around: {}", self.id, obstacle.get_position().as_string()));
                avoid(self.get_position(), target.get_position(), obstacle.position, effective_planet_radius_modifier + obstacle.radius)
            },
            None => {self.calculate_angle_between(target)}
        };
        let thrust_speed = min(speed, distance as i32);
        let velocity_x = thrust_speed as f64 * angle.to_radians().cos();
        let velocity_y = thrust_speed as f64 * angle.to_radians().sin();
        let thrust_end = Position(self.get_position().0 + velocity_x, self.get_position().1 + velocity_y);
        self.velocity_x.set(velocity_x);
        self.velocity_y.set(velocity_y);

        let my_ships = game_map.get_me().all_ships();
        let step_count = 20i32;
        let colliding_ship: Option<&Ship> =
            my_ships.iter()// only ships that could collide this turn need be checked
            .filter(|other| other.id != self.id
                    && self.distance_to(*other) < 2f64 * (SHIP_RADIUS + MAX_SPEED as f64)
                    //&& other.is_undocked()
                   )
            .find(|other|
                  (1..(step_count+1)).collect::<Vec<i32>>().iter()
                  .any(|t|
                       self.dist_to_at(*other, (*t as f64 / step_count as f64).clone()) < (SHIP_RADIUS * 2f64) + fudge
                      )
                 );
        let colliding_ship: Option<Obstacle> = match colliding_ship {
            Some(s) => Some(Obstacle { radius: s.get_radius(), position: s.get_position() }),
            None => None
        };
        let colliding_entity: Option<Obstacle> = match colliding_ship {
            Some(c_s) => Some(c_s),
            None => {
                //game_map.closest_stationary_obstacle(&self.get_position(), &target.get_position(), effective_planet_radius_modifier);
                None
            }
        };

        self.velocity_x.set(0f64);
        self.velocity_y.set(0f64);
        // if collision with other ship X would happen and X is not docked/docking and X has
        // not yet gotten a move order for this turn, return None and try to calculate a new
        // move for self after X has been given orders
        let angular_step = 1.0;
        match colliding_entity {
            Some(other_ship) => {
                for i in 1..(max_corrections + 1) {
                    // try adjusting left and right
                    for angular_offset in vec![i as f64 * angular_step, -1.0 * i as f64 * angular_step] {
                        let new_target_dx = f64::cos((angle + angular_offset).to_radians()) * distance;
                        let new_target_dy = f64::sin((angle + angular_offset).to_radians()) * distance;
                        let Position(self_x, self_y) = self.position;
                        let new_target = Position(self_x + new_target_dx, self_y + new_target_dy);
                        match self.navigate(&new_target, game_map, 0) {
                            Some(command) => {
                                //logger.log(&format!("  ship {} turning by {} to avoid {}", self.id, angular_offset, other_ship.id));
                                return Some(command)
                            },
                            None => {}
                        }
                    }
                }
                None
            },
            None => {
                //logger.log(&format!("  angle: {}", angle as i32));
                Some(self.thrust(thrust_speed, angle as i32))
            }
        }
    }
}

impl PartialEq for Ship {
    fn eq(&self, other: &Ship) -> bool {
        self.id == other.id
    }
}

impl Eq for Ship {}

impl Hash for Ship {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Decodable for Ship {
    fn parse<'a, I>(tokens: &mut I) -> Ship
    where
        I: Iterator<Item = &'a str>,
    {

        let id = i32::parse(tokens);
        let position = Position::parse(tokens);
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
            position,
            hp,
            velocity_x,
            velocity_y,
            docking_status,
            docked_planet,
            progress,
            cooldown,
            command
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
}

impl Planet {
    pub fn is_owned(&self) -> bool {
        self.owner.is_some()
    }

    pub fn open_docks(&self) -> usize {
        self.num_docking_spots as usize - self.docked_ships.len()
    }

    pub fn any_docked(&self) -> bool {
        self.docked_ships.len() > 0
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

pub trait Entity : Sized {
    fn get_position(&self) -> Position;
    fn get_position_at(&self, t: f64) -> Position;
    fn get_radius(&self) -> f64;
    //fn clone(&self) -> Self where Self: Sized;

    fn distance_to<T: Entity>(&self, target: &T) -> f64 {
        let Position(x1, y1) = self.get_position();
        let Position(x2, y2) = target.get_position();
        f64::sqrt((x2-x1).powi(2) + (y2-y1).powi(2))
    }

    fn dist_to_at<T: Entity>(&self, target: &T, t: f64) -> f64 {
        let Position(x1, y1) = self.get_position_at(t);
        let Position(x2, y2) = target.get_position_at(t);
        f64::sqrt((x2-x1).powi(2) + (y2-y1).powi(2))
    }

    fn calculate_angle_between<T: Entity>(&self, target: &T) -> f64 {
        let Position(x1, y1) = self.get_position();
        let Position(x2, y2) = target.get_position();
        (f64::atan2(y2-y1, x2-x1).to_degrees() + 360.0) % 360.0
    }

    fn closest_point_to<T: Entity>(&self, target: &T, min_distance: f64) -> Position {
        let angle = target.calculate_angle_between(self);
        let radius = target.get_radius() + min_distance;
        let Position(target_x, target_y) = target.get_position();
        let x = target_x + radius * f64::cos(angle.to_radians());
        let y = target_y + radius * f64::sin(angle.to_radians());

        Position(x, y)
    }
}

impl Entity for Ship {
    fn get_position(&self) -> Position { self.position }

    fn get_position_at(&self, t: f64) -> Position {
        Position(
            self.position.0 + (t * self.velocity_x.get()),
            self.position.1 + (t * self.velocity_y.get())
            )
    }

    fn get_radius(&self) -> f64 { SHIP_RADIUS }
}

impl Entity for Planet {
    fn get_position(&self) -> Position {
        self.position
    }

    fn get_position_at(&self, t: f64) -> Position {
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

    fn get_position_at(&self, t: f64) -> Position {
        *self
    }

    fn get_radius(&self) -> f64 {
        0.0
    }
}

