
use hlt::entity::{Entity, Position};
macro_rules! in_360 (
    ($angle:expr) => ($angle % 360.0)
    );

fn angle_around(start: Position, destination: Position, obstacle_pos: Position, obstacle_size: f64) -> (f64, f64) {
    // let mut logger = Logger::new(0);
    // s = start position
    // o = obstacle position
    // d = destination position
    // tan = position where trajectory is tangent to obstacle entering arc
    // tan2 = position where trajectory is tangent to obstacle leaving arc
    let d_s_o = start.distance_to(&obstacle_pos);
    // let s_o_d_angle = three_point_angle(start, obstacle_pos, destination);

    // deal with case where ship is inside navigation radius
    let s_o_tan_angle = if obstacle_size > d_s_o {
        (1f64).acos()
    } else {
        (obstacle_size / d_s_o).acos()
    };
    let s_o_tan_angle = s_o_tan_angle.to_degrees();
    let turn_angle = 90.0 - s_o_tan_angle;

    let x_delt = destination.0 - start.0;
    let y_delt = destination.1 - start.1;
    let angle_to_dest = in_360!(y_delt.atan2(x_delt).to_degrees());

    let x_delt = obstacle_pos.0 - start.0;
    let y_delt = obstacle_pos.1 - start.1;
    let angle_to_obstacle = in_360!(y_delt.atan2(x_delt).to_degrees());

    if angle_between(angle_to_dest, in_360!(angle_to_obstacle + turn_angle))
        < angle_between(angle_to_dest, in_360!(angle_to_obstacle - turn_angle))
    {
        (
            in_360!(angle_to_obstacle + turn_angle),
            in_360!(angle_to_obstacle - turn_angle),
        )
    } else {
        (
            in_360!(angle_to_obstacle - turn_angle),
            in_360!(angle_to_obstacle + turn_angle),
        )
    }
}

#[allow(dead_code)]
pub fn long_angle_around(start: Position, destination: Position, obstacle_pos: Position, obstacle_size: f64) -> f64 {
    angle_around(start, destination, obstacle_pos, obstacle_size).1
}

pub fn short_angle_around(start: Position, destination: Position, obstacle_pos: Position, obstacle_size: f64) -> f64 {
    angle_around(start, destination, obstacle_pos, obstacle_size).0
}

fn angle_between(a1: f64, a2: f64) -> f64 {
    let da = (a1 - a2).abs();
    if da > 180.0 {
        180.0 - da
    } else {
        da
    }
}

#[allow(dead_code)]
pub fn three_point_angle(p1: Position, p2: Position, p3: Position) -> f64 {
    let d12 = p1.distance_to(&p2);
    let d13 = p1.distance_to(&p3);
    let d23 = p2.distance_to(&p3);
    ((d12.powi(2) + d13.powi(2) - d23.powi(2)) / (2f64 * d12 * d13)).acos()
}

/* pathfinding idea: take long and short angle around O as two choices,
 * where O is the first object
 * between the ship and its destination. Extend path along that angle until
 * O is no longer the
 * first object between ship and dest. Repeat for additional obstacles.
 * Points where the obstacle
 * between the ship and destination are the graph nodes */
