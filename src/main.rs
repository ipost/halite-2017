//#![cfg_attr(feature = "clippy", feature(plugin))]
//#![cfg_attr(feature = "clippy", plugin(clippy))]

#[macro_use]
mod hlt;

use hlt::entity::{commitment, total_ship_strength, Entity, GameState, Planet, Position, Ship};
use hlt::game::Game;
use hlt::logging::Logger;
use hlt::command::Command;
use std::collections::HashMap;
use hlt::game_map::GameMap;
use hlt::constants::{DEFEND_PREFERENCE_2P, DEFEND_PREFERENCE_4P, DOCK_PREFERENCE_2P, DOCK_PREFERENCE_4P,
                     INTERCEPT_PREFERENCE_2P, INTERCEPT_PREFERENCE_4P, RAID_PREFERENCE_2P, RAID_PREFERENCE_4P,
                     DOCK_RADIUS, DOCK_TURNS, FUDGE, MAX_SPEED, SHIP_RADIUS};
extern crate time;
use time::PreciseTime;
use std::time::Duration;
use std::thread;
use std::cmp::{max, Ordering};

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
}

impl<'a> Move<'a> {
    pub fn value(&self) -> f64 {
        match self {
            &Move::DockMove(_p, v) => v,
            &Move::RaidMove(_s, v) => v,
            &Move::DefendMove(_s, v) => v,
            &Move::InterceptMove(_s, v) => v,
        }
    }

    #[allow(dead_code)]
    pub fn id(&self) -> i32 {
        match self {
            &Move::DockMove(p, _v) => p.id,
            &Move::RaidMove(s, _v) => s.id,
            &Move::DefendMove(s, _v) => s.id,
            &Move::InterceptMove(s, _v) => s.id,
        }
    }

    pub fn move_type(&self) -> MoveType {
        match self {
            &Move::DockMove(_p, _v) => MoveType::DockMove,
            &Move::RaidMove(_s, _v) => MoveType::RaidMove,
            &Move::DefendMove(_s, _v) => MoveType::DefendMove,
            &Move::InterceptMove(_s, _v) => MoveType::InterceptMove,
        }
    }

    pub fn recalculate(
        &mut self,
        ship: &Ship,
        game_map: &GameMap,
        commitment_map: &HashMap<i32, Vec<i32>>,
        configs: &Configs,
    ) {
        match self {
            &mut Move::DockMove(p, ref mut v) => *v = configs.dock_preference * ship.dock_value(p, game_map),
            &mut Move::RaidMove(s, ref mut v) => {
                *v = configs.raid_preference * ship.raid_value(s, game_map, commitment_map)
            }
            &mut Move::DefendMove(s, ref mut v) => {
                *v = configs.defend_preference * ship.defense_value(s, &game_map, commitment_map)
            }
            &mut Move::InterceptMove(s, ref mut v) => {
                *v = configs.intercept_preference * ship.intercept_value(s, commitment_map)
            }
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
        let mut dummy_commitment_map: HashMap<i32, Vec<i32>> = HashMap::new();
        for ship in game_map.enemy_ships() {
            dummy_commitment_map.insert(ship.id, vec![]);
        }
        let dock_moves: Vec<Move> = planets_to_dock
            .into_iter()
            .map(|planet| {
                let mut m = Move::DockMove(planet, 0.0);
                m.recalculate(ship, &game_map, &dummy_commitment_map, &configs);
                m
            })
            .collect();
        let raid_moves: Vec<Move> = enemy_docked_ships
            .into_iter()
            .map(|enemy_ship| {
                let mut m = Move::RaidMove(enemy_ship, 0.0);
                m.recalculate(ship, &game_map, &dummy_commitment_map, &configs);
                m
            })
            .collect();
        // make defend move function of friendly ship? create defend move only if one
        // of closer ships
        let defend_moves: Vec<Move> = enemy_undocked_ships
            .into_iter()
            .map(|enemy_ship| {
                let mut m = Move::DefendMove(enemy_ship, 0.0);
                m.recalculate(ship, &game_map, &dummy_commitment_map, &configs);
                m
            })
            .collect();
        // TODO: disable intercept?
        let intercept_moves: Vec<Move> = enemy_undocked_ships
            .into_iter()
            .map(|enemy_ship| {
                let mut m = Move::InterceptMove(enemy_ship, 0.0);
                m.recalculate(ship, &game_map, &dummy_commitment_map, &configs);
                m
            })
            .collect();
        // let mut intercept_moves: Vec<Move> = vec![];
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

    pub fn recalculate_all_moves(
        &mut self,
        game_map: &GameMap,
        commitment_map: &HashMap<i32, Vec<i32>>,
        configs: &Configs,
    ) {
        for d_m in &mut self.dock_moves {
            d_m.recalculate(self.ship, game_map, commitment_map, &configs);
        }
        for r_m in &mut self.raid_moves {
            r_m.recalculate(self.ship, game_map, commitment_map, &configs);
        }
        for d_m in &mut self.defend_moves {
            d_m.recalculate(self.ship, game_map, commitment_map, &configs);
        }
        for i_m in &mut self.intercept_moves {
            i_m.recalculate(self.ship, game_map, commitment_map, &configs);
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

    #[allow(dead_code)]
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
    let bot_name = "memetron_420v16";
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
        turn_number = turn_number + 1;
        let (game_map1, start_time) = game.update_map(game_map);
        // annoying hack because let is only needed for start_time
        game_map = game_map1;
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

        let enemy_docked_ships: Vec<&Ship> = game_map
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
        // TODO improve this
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

        let mut commitment_map: HashMap<i32, Vec<i32>> = HashMap::new();
        for ship in game_map.enemy_ships() {
            commitment_map.insert(ship.id, vec![]);
        }

        let mut all_ship_moves: Vec<ShipMoves> = vec![];
        for ship in ships_to_order {
            if start_time.to(PreciseTime::now()).num_milliseconds() > 1900 {
                logger.log(&format!(
                    "timeout break in shipmove creation loop {}",
                    start_time.to(PreciseTime::now()).num_milliseconds()
                ));
                break;
            }
            all_ship_moves.push(ShipMoves::new(
                ship,
                &game_map,
                &planets_to_dock,
                &enemy_docked_ships,
                &enemy_undocked_ships,
                &configs,
            ))
        }

        let strongest_enemy_fleet = game_map
            .state
            .players
            .iter()
            .filter(|p| p.id != game.my_id as i32)
            .map(|p| p.all_ships().len())
            .max()
            .unwrap();
        let weakest_enemy_player = game_map
            .state
            .players
            .iter()
            .filter(|p| p.id != game.my_id as i32)
            .filter(|p| p.all_ships().len() > 0)
            .min_by(|p, p2| {
                p.all_ships()
                    .len()
                    .partial_cmp(&p2.all_ships().len())
                    .unwrap()
            })
            .unwrap();
        let should_flee = game_map.state.players.len() > 2
            && strongest_enemy_fleet as f64 > game_map.get_me().all_ships().len() as f64 * 2.0;

        let planet_to_destroy: Option<&Planet> = None; /*{
            let enemy_ships = game_map.enemy_ships();
            let my_ships = game_map.my_ships();
            let mut attackable_planets: Vec<(&Planet, i32)> = game_map
                .all_planets()
                .into_iter()
                .filter(|p| // target not my planets
                        p.owner.is_none() || p.owner.unwrap() != game.my_id as i32)
                .filter(|p| {
                    // filter out planets I couldn't destroy this turn
                    // this is pretty much never possible TT
                    let possible_damage = my_ships
                        .iter()
                        .filter(|s| s.distance_to_surface(*p) < MAX_SPEED as f64)
                        .map(|s| s.hp - s.projected_damage_taken(&game_map))
                        .fold(0, |acc, hp| acc + hp);
                    //disabled for now
                    possible_damage >= p.hp
                })
                .map(|p| {
                    // map of planet, total damage dealt to enemies
                    (
                        p,
                        enemy_ships
                            .iter()
                            .map(|s| p.damage_from_explosion(s))
                            .fold(0, |acc, dmg| acc + dmg),
                    )
                })
                // destroy planet only if damage dealt would be 1.2x cost to destroy
                .filter(|&(p, d)| d as f64 > p.hp as f64 * 0.01)
                .collect();
            let planet = attackable_planets
                .into_iter()
                .max_by(|&(p1, dmg1), &(p2, dmg2)| {
                    (dmg1 as f64 / p1.hp as f64)
                        .partial_cmp(&(dmg2 as f64 / p2.hp as f64))
                        .unwrap()
                });
            match planet {
                Some((p, d)) => {
                    p.doomed.set(true);
                    Some(p)
                }
                None => None,
            }
        };*/
        // when destroying a planet, use its commitment as total hp of my ships
        // committed to its
        // destruction

        let mut commands_issued = 0;
        let mut break_command = -1;
        while game_map.my_ships().iter().any(|s| !s.commanded()) && break_command != commands_issued {
            break_command = commands_issued.clone();

            // next: only recalc if the move would have been affected, which right now
            // should just
            // be if the commitment level of the move target changed
            for s_m in &mut all_ship_moves {
                s_m.recombine_deqs();
                s_m.recalculate_all_moves(&game_map, &commitment_map, &configs);
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
                                flee(ship_to_move.ship, &game_map, &mut logger)
                            // TODO: is this a good idea? maybe eradicate if there's a weaker
                            // enemy, otherwise flee?
                            // eradicate(
                            //     ship_to_move.ship,
                            //     &game_map,
                            //     &mut logger,
                            //     weakest_enemy_player.id,
                            // )
                            } else if planet_to_destroy.is_some() && {
                                let p = planet_to_destroy.unwrap();
                                p.commitment() < p.hp && p.distance_to_surface(ship_to_move.ship) < MAX_SPEED as f64
                            } {
                                kamikaze_planet(ship_to_move.ship, planet_to_destroy.unwrap(), &mut logger)
                            } else {
                                try_move(
                                    ship_to_move,
                                    &game_map,
                                    &enemy_undocked_ships,
                                    &my_docked_ships,
                                    relevant_opponents,
                                    &mut commitment_map,
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
                        if let Command::Thrust(_s_id, speed, angle) = command {
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
                        if attempted_commands[&ship_id] >= max(2000 / ship_count, 30) as i32 {
                            logger.log(&format!("  ship {} will Stay, move not found", ship_id));
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

                if start_time.to(PreciseTime::now()).num_milliseconds() > 1925 {
                    logger.log(&format!(
                        "timeout break in inner loop {}",
                        start_time.to(PreciseTime::now()).num_milliseconds()
                    ));
                    break;
                }
            } // loop
            if start_time.to(PreciseTime::now()).num_milliseconds() > 1900 {
                logger.log(&format!(
                    "timeout break in outer loop {}",
                    start_time.to(PreciseTime::now()).num_milliseconds()
                ));
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

#[allow(dead_code)]
fn flee(ship: &Ship, game_map: &GameMap, logger: &mut Logger) -> Option<Command> {
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
    let speed_angle: Option<(i32, i32)> = ship.smart_navigate(
        &destination,
        &game_map,
        game_map.obstacles_for_flee(ship),
        true,
    );
    match speed_angle {
        Some((speed, angle)) => {
            logger.log(&format!(
                "  ship {} : speed: {}, angle: {} fleeing to {}",
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

fn kamikaze_planet(ship: &Ship, planet: &Planet, logger: &mut Logger) -> Option<Command> {
    let speed = MAX_SPEED;
    let angle = ship.calculate_angle_between(planet).round() as i32;
    planet.committed_ships.set(ship.hp + planet.commitment());
    logger.log(&format!(
        "  ship {} : speed: {}, angle: {}, target planet: {} for death star",
        ship.id,
        speed,
        angle,
        planet.id
    ));
    Some(ship.thrust(speed, angle))
}

fn eradicate(ship: &Ship, game_map: &GameMap, logger: &mut Logger, player_id: i32) -> Option<Command> {
    let enemy_ships: Vec<&Ship> = game_map
        .enemy_ships()
        .into_iter()
        .filter(|s| s.owner_id == player_id)
        .collect();
    let destination = ship.nearest_entity(&enemy_ships).get_position();
    let speed_angle: Option<(i32, i32)> = ship.smart_navigate(
        &destination,
        &game_map,
        game_map.obstacles_for_dock(ship),
        true,
    );
    match speed_angle {
        Some((speed, angle)) => {
            logger.log(&format!(
                "  ship {} : speed: {}, angle: {}, target {} for eradicate player {}",
                ship.id,
                speed,
                angle,
                destination,
                player_id
            ));
            Some(ship.thrust(speed, angle))
        }
        None => None,
    }
}

// TODO form group out of ships committed to same target?
fn try_move(
    ship_to_move: &ShipMoves,
    game_map: &GameMap,
    enemy_undocked_ships: &Vec<&Ship>,
    my_docked_ships: &Vec<&Ship>,
    relevant_opponents: usize,
    commitment_map: &mut HashMap<i32, Vec<i32>>,
    logger: &mut Logger,
) -> Option<Command> {
    let ship = ship_to_move.ship;
    let command = match ship_to_move.best_move() {
        &Move::DockMove(planet, v) => {
            let destination = &ship.closest_point_to(planet, 1.0);
            // check if nearby enemies with commitment == 0
            // problem: commitment is still 0 because defender has not gone yet
            // TODO: maybe move this to dock_value
            let nearby_enemies = enemy_undocked_ships.iter().any(|e_s| {
                e_s.distance_to(destination) < 0.5 * (DOCK_TURNS * MAX_SPEED * 2) as f64
                    && commitment(e_s, commitment_map) < 0.05
            });
            // TODO need cheese defense. if cheesed, move to sit on ally ship?

            // if all dock spots are claimed no command
            // maybe move this to dock_value
            if (planet.num_docking_spots - (planet.committed_ships.get() + planet.docked_ships.len() as i32)) == 0
                || nearby_enemies
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
                logger.log(&format!(
                    "  Ship {} docking to {} value: {}",
                    ship.id,
                    planet.id,
                    v
                ));
                Some(ship.dock(planet))

            // otherwise, fly towards planet
            } else {
                let speed_angle: Option<(i32, i32)> = ship.smart_navigate(
                    destination,
                    &game_map,
                    game_map.obstacles_for_dock(ship),
                    false,
                );
                match speed_angle {
                    Some((speed, angle)) => {
                        logger.log(&format!(
                            "  ship {} : speed: {}, angle: {}, target: {}, target planet: {} value: {}",
                            ship.id,
                            speed,
                            angle,
                            destination,
                            planet.id,
                            v
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

        &Move::RaidMove(enemy_ship, v) => if ship.distance_to_surface(enemy_ship) < MAX_SPEED as f64
            && ship.projected_damage_taken_two_turns(game_map) >= ship.hp
        {
            // kamikaze if ship would die soon
            let destination = enemy_ship.get_position();
            let speed_angle = ship.smart_navigate(
                &destination,
                &game_map,
                game_map.obstacles_for_raid_kamikaze(ship),
                false,
            );
            match speed_angle {
                Some((speed, angle)) => {
                    logger.log(&format!(
                        "  ship {} (hp {}) speed: {}, angle: {}, dest: {} dist: {} will kamikaze to attack ship {} v: {}, PDT: {}",
                        ship.id,
                        ship.hp,
                        speed,
                        angle,
                        destination,
                        ship.distance_to_surface(enemy_ship),
                        enemy_ship.id,
                        v,
                        ship.projected_damage_taken_two_turns(game_map)
                    ));
                    commitment_map
                        .get_mut(&enemy_ship.id)
                        .unwrap()
                        .push(ship.hp);
                    Some(ship.thrust(speed, angle))
                }
                None => None,
            }
        } else {
            let destination = &enemy_ship.get_position();
            let speed_angle: Option<(i32, i32)> = ship.smart_navigate(
                destination,
                &game_map,
                if commitment(enemy_ship, commitment_map)
                    >= total_ship_strength(enemy_ship.defenders(game_map).as_slice())
                {
                    // should recalc moves for ships committed to the target ship if there are less
                    // defenders than ships committed to the target
                    // if raiders outnumber defenders, ignore defenders
                    // game_map.obstacles_for_raid_ignore_defenders(ship, enemy_ship)
                    // game_map.obstacles_for_intercept(ship)
                    game_map.obstacles_for_raid(ship)
                } else {
                    game_map.obstacles_for_raid(ship)
                },
                true,
            );
            match speed_angle {
                Some((speed, angle)) => {
                    logger.log(&format!(
                        "  ship {} (hp {}) : speed: {}, angle: {}, target: {}, dist: {} target ship: {} value: {}, PDT: {}",
                        ship.id,
                        ship.hp,
                        speed,
                        angle,
                        destination,
                        ship.distance_to_surface(enemy_ship),
                        enemy_ship.id,
                        v,
                        ship.projected_damage_taken_two_turns(game_map)
                    ));
                    commitment_map
                        .get_mut(&enemy_ship.id)
                        .unwrap()
                        .push(ship.hp);
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

        &Move::DefendMove(enemy_ship, v) => {
            if my_docked_ships.len() == 0 {
                // if we get here, it probably means we have no docked ships and there
                // aren't any good attack or dock targets. Probably screwed
                Some(Command::Stay())
            } else {
                let ship_to_defend = enemy_ship.nearest_entity(my_docked_ships.as_slice());

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
                        // TODO: what is the best strategy here
                        // (enemy_ship.get_position().0 + ship_to_defend.get_position().0) / 2.0,
                        // (enemy_ship.get_position().1 + ship_to_defend.get_position().1) / 2.0,
                        // (ship_to_defend.get_position().0 + (dx / magnitude)),
                        // (ship_to_defend.get_position().1 + (dy / magnitude)),
                        (enemy_ship.get_position().0 - (dx / magnitude)),
                        (enemy_ship.get_position().1 - (dy / magnitude)),
                    )
                };
                let speed_angle: Option<(i32, i32)> = ship.smart_navigate(
                    &destination,
                    &game_map,
                    game_map.obstacles_for_defend(ship),
                    true,
                );
                match speed_angle {
                    Some((speed, angle)) => {
                        logger.log(&format!(
                            "  ship {} : speed: {}, angle: {}, target: {}, defending {} from: {} value: {}",
                            ship.id,
                            speed,
                            angle,
                            destination,
                            ship_to_defend.id,
                            enemy_ship.id,
                            v
                        ));
                        commitment_map
                            .get_mut(&enemy_ship.id)
                            .unwrap()
                            .push(ship.hp);
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

        &Move::InterceptMove(enemy_ship, v) => {
            // TODO: move to enemy projected position?
            let destination = enemy_ship.get_position();
            let speed_angle: Option<(i32, i32)> = ship.smart_navigate(
                &destination,
                &game_map,
                game_map.obstacles_for_intercept(ship),
                true,
            );
            match speed_angle {
                Some((speed, angle)) => {
                    logger.log(&format!(
                        "  ship {} : speed: {}, angle: {}, target: {}, intercepting {} value: {}",
                        ship.id,
                        speed,
                        angle,
                        destination,
                        enemy_ship.id,
                        v
                    ));
                    commitment_map
                        .get_mut(&enemy_ship.id)
                        .unwrap()
                        .push(ship.hp);
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
    };
    command
}
