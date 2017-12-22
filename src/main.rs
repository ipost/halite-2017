#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

mod hlt;

use hlt::entity::{DockingStatus, Entity, GameState, Planet, Position, Ship};
use hlt::game::Game;
use hlt::logging::Logger;
use hlt::command::Command;
use hlt::macros::*;
use std::collections::HashMap;
macro_rules! assert_unreachable (
    () => { panic!(format!("line {}", line!())) }
    );
use hlt::game_map::GameMap;
use hlt::constants::{DEFEND_PREFERENCE_2P, DEFEND_PREFERENCE_4P, DOCK_PREFERENCE_2P, DOCK_PREFERENCE_4P,
                     INTERCEPT_PREFERENCE_2P, INTERCEPT_PREFERENCE_4P, RAID_PREFERENCE_2P, RAID_PREFERENCE_4P,
                     DOCK_RADIUS, DOCK_TURNS, FUDGE, MAX_CORRECTIONS, MAX_SPEED, SHIP_RADIUS, WEAPON_RADIUS};
extern crate time;
use time::PreciseTime;
use std::cmp::Ordering;

macro_rules! assert_unreachable (
    () => { panic!(format!("line {}", line!())) }
    );
macro_rules! print_timing (
        ($code: block) => {{
            let pt_start_time = PreciseTime::now();
            let res = $code;
            Logger::new(0).log(&format!("  time at line {}: {}", line!(), pt_start_time.to(PreciseTime::now())));
            res
        }}
            );

macro_rules! in_360 (
    ($angle:expr) => (($angle + 360.0) % 360.0)
    );

#[derive(Debug)]
struct Configs {
    dock_preference: f64,
    raid_preference: f64,
    defend_preference: f64,
    intercept_preference: f64,
}

#[derive(Debug)]
enum MoveType {
    DockMove,
    RaidMove,
    DefendMove,
    InterceptMove,
}

#[derive(Debug)]
enum Move<'a> {
    DockMove(&'a Planet, f64),
    RaidMove(&'a Ship, f64),
    DefendMove(&'a Ship, f64),
    InterceptMove(&'a Ship, f64),
    //TODO: move to destroy planet when enemy is clumped around it!
}

impl<'a> Move<'a> {
    pub fn value(&self) -> f64 {
        match self {
            &Move::DockMove(p, v) => v,
            &Move::RaidMove(s, v) => v,
            &Move::DefendMove(s, v) => v,
            &Move::InterceptMove(s, v) => v,
        }
    }

    pub fn id(&self) -> i32 {
        match self {
            &Move::DockMove(p, v) => p.id,
            &Move::RaidMove(s, v) => s.id,
            &Move::DefendMove(s, v) => s.id,
            &Move::InterceptMove(s, v) => s.id,
        }
    }

    pub fn move_type(&self) -> MoveType {
        match self {
            &Move::DockMove(p, v) => MoveType::DockMove,
            &Move::RaidMove(s, v) => MoveType::RaidMove,
            &Move::DefendMove(s, v) => MoveType::DefendMove,
            &Move::InterceptMove(s, v) => MoveType::InterceptMove,
        }
    }

    // pub fn commitment(&self) -> i32 {
    //    match self {
    //        //&Move::DockMove(p, v) => p.committed_ships.get(),
    //        &Move::DockMove(p, v) => 0,
    //        &Move::RaidMove(s, v) => s.committed_ships.get(),
    //        &Move::DefendMove(s, v) => s.committed_ships.get(),

    //        // do not intercept until we defend and raid
    //        &Move::InterceptMove(s, v) => s.committed_ships.get() + 1,
    //    }
    // }

    pub fn recalculate(&mut self, ship: &Ship, game_map: &GameMap, configs: &Configs) {
        match self {
            &mut Move::DockMove(p, ref mut v) => *v = configs.dock_preference * ship.dock_value(p, game_map),
            &mut Move::RaidMove(s, ref mut v) => *v = configs.raid_preference * ship.raid_value(s),
            &mut Move::DefendMove(s, ref mut v) => *v = configs.defend_preference * ship.defense_value(s, &game_map),
            &mut Move::InterceptMove(s, ref mut v) => *v = configs.intercept_preference * ship.intercept_value(s),
        }
    }
}

#[derive(Debug)]
struct ShipMoves<'a> {
    ship: &'a Ship,
    dock_moves: Vec<Move<'a>>,
    raid_moves: Vec<Move<'a>>,
    defend_moves: Vec<Move<'a>>,
    intercept_moves: Vec<Move<'a>>,
    deqd_dock_moves: Vec<Move<'a>>,
    deqd_raid_moves: Vec<Move<'a>>,
    deqd_defend_moves: Vec<Move<'a>>,
    deqd_intercept_moves: Vec<Move<'a>>,
    best_move: MoveType,
}

impl<'a> ShipMoves<'a> {
    pub fn new<'b>(
        ship: &'b Ship,
        game_map: &'b GameMap,
        planets_to_dock: &Vec<&'b Planet>,
        enemy_docked_ships: &Vec<&'b Ship>,
        enemy_undocked_ships: &Vec<&'b Ship>,
        configs: &Configs,
    ) -> ShipMoves<'b> {
        let mut dock_moves: Vec<Move> = planets_to_dock
            .into_iter()
            .map(|p| Move::DockMove(p, 0.0))
            .collect();
        let mut raid_moves: Vec<Move> = enemy_docked_ships
            .into_iter()
            .map(|enemy_ship| Move::RaidMove(enemy_ship, 0.0))
            .collect();
        // make defend move function of friendly ship? create defend move only if one
        // of closer ships
        let mut defend_moves: Vec<Move> = enemy_undocked_ships
            .into_iter()
            .map(|enemy_ship| Move::DefendMove(enemy_ship, 0.0))
            .collect();
        let mut intercept_moves: Vec<Move> = enemy_undocked_ships
            .into_iter()
            .map(|enemy_ship| Move::InterceptMove(enemy_ship, 0.0))
            .collect();
        for d_m in &mut dock_moves {
            d_m.recalculate(ship, &game_map, &configs);
        }
        for r_m in &mut raid_moves {
            r_m.recalculate(ship, &game_map, &configs);
        }
        for d_m in &mut defend_moves {
            d_m.recalculate(ship, &game_map, &configs);
        }
        for i_m in &mut intercept_moves {
            i_m.recalculate(ship, &game_map, &configs);
        }
        let deqd_dock_moves = vec![];
        let deqd_raid_moves = vec![];
        let deqd_defend_moves = vec![];
        let deqd_intercept_moves = vec![];
        let best_move = MoveType::DockMove;
        let mut s_m = ShipMoves {
            ship,
            dock_moves,
            raid_moves,
            defend_moves,
            intercept_moves,
            deqd_dock_moves,
            deqd_raid_moves,
            deqd_defend_moves,
            deqd_intercept_moves,
            best_move,
        };
        s_m.sort_moves();
        s_m.refresh_best_move();
        s_m
    }

    // moves must be sorted by value within their type before calling
    pub fn update_best_move(&mut self) {
        match self.best_move {
            MoveType::DockMove => self.deqd_dock_moves.push(self.dock_moves.remove(0)),
            MoveType::RaidMove => self.deqd_raid_moves.push(self.raid_moves.remove(0)),
            MoveType::DefendMove => self.deqd_defend_moves.push(self.defend_moves.remove(0)),
            MoveType::InterceptMove => self.deqd_intercept_moves
                .push(self.intercept_moves.remove(0)),
        };
        self.refresh_best_move();
    }

    pub fn recombine_deqs(&mut self) {
        while self.deqd_dock_moves.len() > 0 {
            self.dock_moves.push(self.deqd_dock_moves.remove(0));
        }
        while self.deqd_raid_moves.len() > 0 {
            self.raid_moves.push(self.deqd_raid_moves.remove(0));
        }
        while self.deqd_defend_moves.len() > 0 {
            self.defend_moves.push(self.deqd_defend_moves.remove(0));
        }
        while self.deqd_intercept_moves.len() > 0 {
            self.intercept_moves
                .push(self.deqd_intercept_moves.remove(0));
        }
    }

    pub fn recalculate_all_moves(&mut self, game_map: &GameMap, configs: &Configs) {
        for d_m in &mut self.dock_moves {
            d_m.recalculate(self.ship, &game_map, &configs);
        }
        for r_m in &mut self.raid_moves {
            r_m.recalculate(self.ship, &game_map, &configs);
        }
        for d_m in &mut self.defend_moves {
            d_m.recalculate(self.ship, &game_map, &configs);
        }
        for i_m in &mut self.intercept_moves {
            i_m.recalculate(self.ship, &game_map, &configs);
        }
    }

    pub fn refresh_best_move(&mut self) {
        self.best_move = {
            vec![
                self.dock_moves.first(),
                self.raid_moves.first(),
                self.defend_moves.first(),
                self.intercept_moves.first(),
            ].into_iter()
                .filter(|m| m.is_some())
                .map(|m| m.unwrap())
                .min_by(|move1, move2| {
                    move1.value().partial_cmp(&move2.value()).unwrap()
                })
                .unwrap()
                .move_type()
        };
    }

    pub fn sort_moves(&mut self) {
        let sort_fn = |m1: &Move, m2: &Move| -> Ordering { m1.value().partial_cmp(&m2.value()).unwrap() };
        self.dock_moves.sort_by(&sort_fn);
        self.raid_moves.sort_by(&sort_fn);
        self.defend_moves.sort_by(&sort_fn);
        self.intercept_moves.sort_by(&sort_fn);
    }

    pub fn remaining_moves(&self) -> usize {
        self.dock_moves.len() + self.raid_moves.len() + self.defend_moves.len() + self.intercept_moves.len()
    }

    pub fn best_move(&self) -> &Move {
        match self.best_move {
            MoveType::DockMove => self.dock_moves.first().unwrap(),
            MoveType::RaidMove => self.raid_moves.first().unwrap(),
            MoveType::DefendMove => self.defend_moves.first().unwrap(),
            MoveType::InterceptMove => self.intercept_moves.first().unwrap(),
        }
    }

    pub fn as_string(&self) -> String {
        format!(
            "
ShipMoves {{
    ship_id: {}
    best_move: {:#?}
    dock_moves: {}
    raid_moves: {}
    defend_moves: {}
    intercept_moves: {}
    }}",
            self.ship.id,
            self.best_move(),
            self.dock_moves
                .iter()
                .map(|m| format!("planet_id: {}, value: {}", m.id(), m.value()))
                .fold(String::new(), |acc, s| { acc + "\n        " + &s }),
            self.raid_moves
                .iter()
                .map(|m| format!("ship_id: {}, value: {}", m.id(), m.value()))
                .fold(String::new(), |acc, s| { acc + "\n        " + &s }),
            self.defend_moves
                .iter()
                .map(|m| format!("ship_id: {}, value: {}", m.id(), m.value()))
                .fold(String::new(), |acc, s| { acc + "\n        " + &s }),
            self.intercept_moves
                .iter()
                .map(|m| format!("ship_id: {}, value: {}", m.id(), m.value()))
                .fold(String::new(), |acc, s| { acc + "\n        " + &s }),
        )
    }
}

fn main() {
    // Initialize the game
    let bot_name = "memetron_420v8";
    let game = Game::new(bot_name);
    // Initialize logging
    let mut logger = Logger::new(game.my_id);
    logger.log(&format!("Starting my {} bot!", bot_name));

    // For each turn
    let mut turn_number: usize = 0;
    let gs = GameState {
        players: vec![],
        planets: vec![],
    };
    let mut game_map = GameMap::new(&game, gs);
    loop {
        let start_time = PreciseTime::now();
        turn_number = turn_number + 1;
        // Update the game state
        game_map = game.update_map(game_map);
        let mut command_queue: Vec<Command> = Vec::new();

        // set playercount-dependent params
        let my_ship_count = game_map.get_me().all_ships().len();
        let relevant_opponents = game_map
            .state
            .players
            .iter()
            .filter(|p| p.id != game.my_id as i32)
            .filter(|p| p.all_ships().len() * 2 > my_ship_count)
            .count();
        let (dock_preference, raid_preference, defend_preference, intercept_preference) = if relevant_opponents > 1 {
            (
                DOCK_PREFERENCE_4P,
                RAID_PREFERENCE_4P,
                DEFEND_PREFERENCE_4P,
                INTERCEPT_PREFERENCE_4P,
            )
        } else {
            (
                DOCK_PREFERENCE_2P,
                RAID_PREFERENCE_2P,
                DEFEND_PREFERENCE_2P,
                INTERCEPT_PREFERENCE_2P,
            )
        };
        let configs = Configs {
            dock_preference,
            raid_preference,
            defend_preference,
            intercept_preference,
        };

        let ships = game_map.get_me().all_ships();
        {
            let ship_ids = ships
                .iter()
                .map(|s| s.id.to_string())
                .collect::<Vec<String>>()
                .join(" ");
            logger.log(&format!("turn {}, my ships: {}", turn_number, ship_ids));
        }

        let planets_to_dock: Vec<&Planet> = game_map
            .all_planets()
            .iter()
            .filter(|p| {
                !p.is_owned() || (p.is_owned() && p.owner.unwrap() == game.my_id as i32 && p.open_docks() > 0)
            })
            .collect();

        let mut enemy_docked_ships: Vec<&Ship> = game_map
            .enemy_ships()
            .into_iter()
            .filter(|s| !s.is_undocked())
            .collect();

        let enemy_undocked_ships: Vec<&Ship> = game_map
            .enemy_ships()
            .into_iter()
            .filter(|s| s.is_undocked())
            .collect();

        // predict enemy ship movement
        let my_ships = game_map.my_ships();
        for s in enemy_undocked_ships.iter() {
            let my_closest = s.nearest_entity(my_ships.as_slice());
            let (speed, angle) = s.route_to(my_closest, &game_map);
            let velocity_x = speed as f64 * (angle as f64).to_radians().cos();
            let velocity_y = speed as f64 * (angle as f64).to_radians().sin();
            s.set_velocity(velocity_x, velocity_y);
        }

        let ship_count = my_ships.len();
        let my_docked_ships: Vec<&Ship> = my_ships.into_iter().filter(|s| !s.is_undocked()).collect();

        let mut ships_to_order = vec![];
        let mut attempted_commands: HashMap<i32, i32> = HashMap::new();
        // Ignore ships that are in the process of (un)docking
        for ship in ships {
            if ship.is_undocked() {
                attempted_commands.insert(ship.id, 0);
                ships_to_order.push(ship);
            } else if !ship.is_docked() {
                logger.log(&format!(
                    "  ship {} will remain {}",
                    ship.id,
                    ship.docking_status
                ));
                ship.command.set(Some(Command::Stay()));
            }
        }

        let mut all_ship_moves: Vec<ShipMoves> = ships_to_order
            .into_iter()
            .map(|ship| {
                ShipMoves::new(
                    ship,
                    &game_map,
                    &planets_to_dock,
                    &enemy_docked_ships,
                    &enemy_undocked_ships,
                    &configs,
                )
            })
            .collect();

        let strongest_enemy_fleet = game_map
            .state
            .players
            .iter()
            .filter(|p| p.id != game.my_id as i32)
            .map(|p| p.all_ships().len())
            .max()
            .unwrap();
        let should_flee =
            game_map.state.players.len() > 2 && strongest_enemy_fleet > game_map.get_me().all_ships().len() * 3;

        let mut commands_issued = 0;
        let mut break_command = -1;
        while game_map.my_ships().iter().any(|s| !s.commanded()) && break_command != commands_issued {
            break_command = commands_issued.clone();

            // next: only recalc if the move would have been affected, which right now
            // should just
            // be if the commitment level of the move target changed
            for s_m in &mut all_ship_moves {
                s_m.recombine_deqs();
                s_m.recalculate_all_moves(&game_map, &configs);
                s_m.sort_moves();
                s_m.refresh_best_move();
            }

            // break executed at end if command issued
            loop {
                let (ship_id, command) = {
                    // command docked ship
                    if let Some(ship) = game_map
                        .my_ships()
                        .iter()
                        .find(|s| !s.commanded() && s.is_docked())
                    {
                        if should_flee {
                            logger.log(&format!("  ship {} will undock to flee", ship.id));
                            (ship.id, Some(ship.undock()))
                        } else {
                            logger.log(&format!("  ship {} will remain DOCKED", ship.id));
                            (ship.id, Some(Command::Stay()))
                        }

                    // find the current undocked ship which has the best move to make
                    } else if let Some(ship_to_move) = all_ship_moves
                        .iter()
                            // ?????
                            //.filter(|s_m| s_m.remaining_moves() > 0)
                            .filter(|s_m| s_m.remaining_moves() > 1)
                            .min_by(|s_m1, s_m2| {
                                s_m1.best_move()
                                    .value()
                                    .partial_cmp(&s_m2.best_move().value())
                                    .unwrap()
                            }) {
                        (
                            ship_to_move.ship.id,
                            if should_flee {
                                flee(
                                    ship_to_move.ship,
                                    &game_map,
                                    &enemy_undocked_ships,
                                    &mut logger,
                                )
                            } else if false {
                                // planet destruction: select some ships to crash into planet
                                // move others out of explosion radius
                                None
                            } else {
                                // maybe don't safely adjust when having ship advantage? 1.5:1?
                                try_make_move(
                                    ship_to_move,
                                    &game_map,
                                    &enemy_undocked_ships,
                                    &my_docked_ships,
                                    relevant_opponents,
                                    &mut attempted_commands,
                                    &mut logger,
                                )
                            },
                        )

                    // there are no ships left to command
                    } else {
                        break;
                    }
                };

                match command {
                    Some(command) => {
                        match command {
                            Command::Stay() => {}
                            _ => command_queue.push(command),
                        }
                        let ship: &Ship = game_map.get_ship(ship_id);
                        ship.command.set(Some(command));
                        match all_ship_moves.iter().position(|s_m| s_m.ship.id == ship.id) {
                            Some(index) => {
                                all_ship_moves.remove(index);
                            }
                            None => {}
                        };
                        if let Command::Thrust(s_id, speed, angle) = command {
                            ship.set_velocity(
                                speed as f64 * (angle as f64).to_radians().cos(),
                                speed as f64 * (angle as f64).to_radians().sin(),
                            );
                        }
                        commands_issued += 1;
                        break;
                    }
                    None => if attempted_commands.contains_key(&ship_id) {
                        *attempted_commands.get_mut(&ship_id).unwrap() += 1;
                        if attempted_commands[&ship_id] >= (12000 / ship_count) as i32 {
                            logger.log(&format!("  ship {} will Stay, move not found", ship_id,));
                            game_map
                                .get_ship(ship_id)
                                .command
                                .set(Some(Command::Stay()));
                            let index = all_ship_moves
                                .iter()
                                .position(|s_m| s_m.ship.id == ship_id)
                                .unwrap();
                            all_ship_moves.remove(index);
                            commands_issued += 1;
                            break;
                        }
                        all_ship_moves
                            .iter_mut()
                            .find(|s_m| s_m.ship.id == ship_id)
                            .unwrap()
                            .update_best_move();
                    },
                }
            } // loop
            if start_time.to(PreciseTime::now()).num_milliseconds() > 1900 {
                break;
            }
        }
        for command in command_queue.iter() {
            logger.log(&format!("{}", command.encode()));
        }
        game.send_command_queue(command_queue);
        logger.log(&format!(
            "  turn time: {}\n\n",
            start_time.to(PreciseTime::now())
        ));
    }
}

fn flee(ship: &Ship, game_map: &GameMap, enemy_undocked_ships: &Vec<&Ship>, logger: &mut Logger) -> Option<Command> {
    let margin = 1.7;
    let small_margin = SHIP_RADIUS + FUDGE;
    let center = game_map.center();
    let ship_angle: f64 = in_360!(
        (ship.get_position().1 - center.1)
            .atan2(ship.get_position().0 - center.0)
            .to_degrees()
    );
    let north_range = (
        in_360!((-1.0 * center.1).atan2(-1.0 * center.0).to_degrees()),
        in_360!((-1.0 * center.1).atan2(1.0 * center.0).to_degrees()),
    );
    let south_range = (
        in_360!((1.0 * center.1).atan2(1.0 * center.0).to_degrees()),
        in_360!((1.0 * center.1).atan2(-1.0 * center.0).to_degrees()),
    );
    let west_range = (
        in_360!((1.0 * center.1).atan2(-1.0 * center.0).to_degrees()),
        in_360!((-1.0 * center.1).atan2(-1.0 * center.0).to_degrees()),
    );
    let destination = if ship_angle <= south_range.1 && ship_angle >= south_range.0 {
        Position {
            0: game_map.width() - small_margin,
            1: game_map.height() - margin,
        }
    } else if ship_angle < west_range.1 && ship_angle > west_range.0 {
        Position {
            0: margin,
            1: game_map.height() - small_margin,
        }
    } else if ship_angle < north_range.1 && ship_angle > north_range.0 {
        Position {
            0: small_margin,
            1: margin,
        }
    } else {
        Position {
            0: game_map.width() - margin,
            1: small_margin,
        }
    };
    let (speed, angle) = ship.route_to(&destination, &game_map);
    match ship.safely_adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS) {
        Some((speed, angle)) => {
            logger.log(&format!(
                "  ship {} : speed: {}, angle: {}, target: {}",
                ship.id,
                speed,
                angle,
                destination,
            ));
            Some(ship.thrust(speed, angle))
        }
        _ => {
            logger.log(&format!(
                "  --- failed to find path to flee for ship {}",
                ship.id
            ));
            None
        }
    }
}

fn try_make_move(
    ship_to_move: &ShipMoves,
    game_map: &GameMap,
    enemy_undocked_ships: &Vec<&Ship>,
    my_docked_ships: &Vec<&Ship>,
    relevant_opponents: usize,
    attempted_commands: &mut HashMap<i32, i32>,
    logger: &mut Logger,
) -> Option<Command> {
    let ship = ship_to_move.ship;
    let command = match ship_to_move.best_move() {
        // execute dock move
        &Move::DockMove(planet, v) => {
            let destination = &ship.closest_point_to(planet, 1.0);
            // check if nearby enemies with commitment == 0
            // TODO: maybe move this to dock_value
            let nearby_enemies = enemy_undocked_ships.iter().any(|e_s| {
                e_s.distance_to(destination) < (DOCK_TURNS * MAX_SPEED * 2) as f64 && e_s.committed_ships.get() == 0
            });

            // if all dock spots are claimed no command
            // maybe move this to dock_value
            if (planet.num_docking_spots - (planet.committed_ships.get() + planet.docked_ships.len() as i32)) == 0
            //|| nearby_enemies
            {
                None

            // if a ship would spawn before we could arrive
            } else if (planet.turns_until_spawn() as f64)
                < (ship.distance_to_surface(planet) + DOCK_RADIUS) / MAX_SPEED as f64
            {
                None

            // if close enough to dock
            } else if ship.in_dock_range(planet) {
                planet.committed_ships.set(planet.committed_ships.get() + 1);
                logger.log(&format!("  Ship {} docking to {}", ship.id, planet.id));
                Some(ship.dock(planet))

            // otherwise, fly towards planet
            } else {
                let (speed, angle) = ship.route_to(destination, &game_map);
                let speed_angle: Option<(i32, i32)> = if attempted_commands.get(&ship.id).unwrap() < &25 {
                    ship.safely_adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS)
                } else {
                    ship.adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS)
                };
                match speed_angle {
                    Some((speed, angle)) => {
                        logger.log(&format!(
                            "  ship {} : speed: {}, angle: {}, target: {}, target planet: {}",
                            ship.id,
                            speed,
                            angle,
                            destination,
                            planet.id
                        ));
                        planet.increment_committed_ships();
                        Some(ship.thrust(speed, angle))
                    }
                    _ => {
                        logger.log(&format!(
                            "  --- failed to find path to planet {} for ship {}",
                            planet.id,
                            ship.id
                        ));
                        None
                    }
                }
            }
        }

        // execute raid move
        &Move::RaidMove(enemy_ship, v) => if ship.distance_to(enemy_ship) < WEAPON_RADIUS / 2.0 {
            // TODO: run away when attacked?
            logger.log(&format!(
                "  ship {} will remain {} to attack {}",
                ship.id,
                ship.docking_status,
                enemy_ship.id
            ));
            Some(Command::Stay())
        } else {
            let destination = &ship.closest_point_to(enemy_ship, WEAPON_RADIUS * 0.95);
            let (speed, angle) = ship.route_to(destination, &game_map);
            let speed_angle: Option<(i32, i32)> = if attempted_commands.get(&ship.id).unwrap() < &25 {
                ship.safely_adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS)
            } else {
                ship.adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS)
            };
            // maybe don't safely adjust after hitting some threshold of attempted
            // commands? or maybe if surrounded by too many friendlies
            // or maybe try taking the long way around the obstacle, if any
            match speed_angle {
                Some((speed, angle)) => {
                    if speed == 0 {
                        logger.log(&format!(
                            "This shouldn't happen. The ship should remain to attack instead if it's that close. I think?"
                        ));
                    }
                    logger.log(&format!(
                        "  ship {} : speed: {}, angle: {}, target: {}, target ship: {}",
                        ship.id,
                        speed,
                        angle,
                        destination,
                        enemy_ship.id
                    ));
                    enemy_ship.increment_committed_ships();
                    Some(ship.thrust(speed, angle))
                }
                _ => {
                    logger.log(&format!(
                        "  --- failed to find path to ship {} for ship {}",
                        enemy_ship.id,
                        ship.id
                    ));
                    None
                }
            }
        },

        // execute defend move
        &Move::DefendMove(enemy_ship, v) => {
            if my_docked_ships.len() == 0 {
                // if we get here, it probably means we have no docked ships and there
                // aren't any good attack or dock targets. Probably screwed
                Some(Command::Stay())
            } else {
                let ship_to_defend = enemy_ship.nearest_entity(my_docked_ships.as_slice());

                // kamikaze behavior?
                if enemy_ship.hp - 100 > ship.hp {
                    let destination = enemy_ship.get_position();
                    let (speed, angle) = ship.route_to(&destination, &game_map);
                    logger.log(&format!(
                        "  ship {} : speed: {}, angle: {}, target: {}, defending {} from: {} via KAMIKAZE",
                        ship.id,
                        speed,
                        angle,
                        destination,
                        ship_to_defend.id,
                        enemy_ship.id
                    ));
                    enemy_ship.increment_committed_ships();
                    Some(ship.thrust(speed, angle))
                } else {
                    let (dx, dy) = (
                        (enemy_ship.get_position().0 - ship_to_defend.get_position().0),
                        (enemy_ship.get_position().1 - ship_to_defend.get_position().1),
                    );
                    let magnitude = f64::sqrt(dx.powi(2) + dy.powi(2));
                    let destination = if relevant_opponents > 1 {
                        Position(
                            (ship_to_defend.get_position().0 + (dx / magnitude)),
                            (ship_to_defend.get_position().1 + (dy / magnitude)),
                        )
                    } else {
                        Position(
                            (enemy_ship.get_position().0 - (dx / magnitude)),
                            (enemy_ship.get_position().1 - (dy / magnitude)),
                        )
                    };
                    let (speed, angle) = ship.route_to(&destination, &game_map);
                    let speed_angle: Option<(i32, i32)> = ship.adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS);
                    match speed_angle {
                        Some((speed, angle)) => {
                            logger.log(&format!(
                                "  ship {} : speed: {}, angle: {}, target: {}, defending {} from: {}",
                                ship.id,
                                speed,
                                angle,
                                destination,
                                ship_to_defend.id,
                                enemy_ship.id
                            ));
                            enemy_ship.increment_committed_ships();
                            Some(ship.thrust(speed, angle))
                        }
                        _ => {
                            logger.log(&format!(
                                "  --- failed to find path to ship {} for ship {}",
                                enemy_ship.id,
                                ship.id
                            ));
                            None
                        }
                    }
                }
            }
        }

        // execute intercept move
        &Move::InterceptMove(enemy_ship, v) => {
            // TODO: move to enemy projected position?
            let destination = enemy_ship.get_position();
            let (speed, angle) = ship.route_to(&destination, &game_map);
            let speed_angle: Option<(i32, i32)> = ship.adjust_thrust(&game_map, speed, angle, MAX_CORRECTIONS);
            match speed_angle {
                Some((speed, angle)) => {
                    logger.log(&format!(
                        "  ship {} : speed: {}, angle: {}, target: {}, intercepting {}",
                        ship.id,
                        speed,
                        angle,
                        destination,
                        enemy_ship.id
                    ));
                    enemy_ship.increment_committed_ships();
                    Some(ship.thrust(speed, angle))
                }
                _ => {
                    logger.log(&format!(
                        "  --- failed to find path to ship {} for ship {}",
                        enemy_ship.id,
                        ship.id
                    ));
                    None
                }
            }
        }
        _ => assert_unreachable!(),
    };
    command
}
