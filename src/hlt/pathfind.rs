
use hlt::entity::{Entity, Position};
//use hlt::constants::{SHIP_RADIUS};
use std::f64::consts::PI;
//use hlt::logging::Logger;

pub fn avoid(start: Position,
             destination: Position,
             obstacle_pos: Position,
             obstacle_size: f64
            ) -> f64 {
    //let mut logger = Logger::new(0);
    // s = start position
    // o = obstacle position
    // d = destination position
    // tan = position where trajectory is tangent to obstacle entering arc
    // tan2 = position where trajectory is tangent to obstacle leaving arc
    let d_s_o = start.distance_to(&obstacle_pos);
    //let s_o_d_angle = three_point_angle(start, obstacle_pos, destination);

    //deal with case where ship is inside navigation radius
    let s_o_tan_angle = if obstacle_size > d_s_o {
        (1f64).acos()
    } else {
        (obstacle_size / d_s_o).acos()
    };
    //logger.log(&format!("obstacle_size: {}, d_s_o: {}, s_o_tan_angle: {}, s_o_d_angle: {}", obstacle_size, d_s_o, s_o_tan_angle, s_o_d_angle));
    let turn_angle = (PI / 2f64) - s_o_tan_angle;

    let x_delt = destination.0 - start.0;
    let y_delt = destination.1 - start.1;
    let angle_to_dest = y_delt.atan2(x_delt);

    let x_delt = obstacle_pos.0 - start.0;
    let y_delt = obstacle_pos.1 - start.1;
    let angle_to_obstacle = y_delt.atan2(x_delt);

    let angle = if (angle_to_dest - (angle_to_obstacle + turn_angle)).abs() < 
        (angle_to_dest - (angle_to_obstacle - turn_angle)) {
            (angle_to_obstacle + turn_angle)
        } else {
            (angle_to_obstacle - turn_angle)
        };
    (angle.to_degrees() + 360.0) % 360.0
}

#[allow(dead_code)]
pub fn three_point_angle(p1: Position, p2: Position, p3: Position) -> f64{
    let d12 = p1.distance_to(&p2);
    let d13 = p1.distance_to(&p3);
    let d23 = p2.distance_to(&p3);
    ((d12.powi(2) + d13.powi(2) - d23.powi(2)) /
     (2f64 * d12 * d13)
    ).acos()
}


/*
pub fn pathfind<T: Entity>(ship: Ship, to: Position, game_map: &GameMap) -> Option<Command> {
    None
}

pub fn distance_around_obstacle(start: Position,
                                destination: Position,
                                obstacle_pos: Position,
                                obstacle_size: f64
                                ) -> f64 {
    // s = start position
    // o = obstacle position
    // d = destination position
    // tan = position where trajectory is tangent to obstacle entering arc
    // tan2 = position where trajectory is tangent to obstacle leaving arc
    let d_s_d = start.distance_to(&destination);
    let d_s_o = start.distance_to(&obstacle_pos);
    let d_o_d = obstacle_pos.distance_to(&destination);
    let total_radius = obstacle_size + 0.05;//+ SHIP_SIZE
    let s_o_d_angle = three_point_angle(start, obstacle_pos, destination);
    let s_o_tan_angle = (total_radius / d_s_o).acos();
    let d_o_tan2_angle = (total_radius / d_o_d).acos();

    // distance to tangent point on obstacle +
    // distance second tangent point on obstacle to destination +
    // length of arc between tangent points
    ((total_radius / d_s_o).asin().cos() * d_s_o) +
        ((total_radius / d_o_d).asin().cos() * d_o_d) +
        ((s_o_d_angle - (s_o_tan_angle + d_o_tan2_angle)) * total_radius)
}                       

//A* component
struct Path {
    start: Position,
    destination: Position,
    steps: Vec<Position>,
    length: f64,
}
impl Path {
    fn heuristic(&self) -> f64 {
        self.pos().distance_to(&self.destination)
    }

    fn pos(&self) -> Position {
        match self.steps.last() {
            Some(pos) => *pos,
            None => self.start,
        }
    }

    // if true, path is complete
    pub fn attach_next_step(&self, game_map: &GameMap) -> bool {
        match game_map.closest_obstacle(&self.pos(), &self.destination, SHIP_RADIUS) {
            Some((pos, radius)) => {
                //route around pos
            },
            None => {
                self.steps.push(self.destination);
                return true
            }
        }
    }

    pub fn cost(&self) -> f64 {
        self.heuristic() + self.length
    }
}
*/
