
use hlt::entity::{Entity, Position};

/*
   Test whether a line segment and circle intersect.
   :param Entity start: The start of the line segment. (Needs x, y attributes)
   :param Entity end: The end of the line segment. (Needs x, y attributes)
   :param Entity circle: The circle to test against. (Needs x, y, r attributes)
   :param float fudge: A fudge factor; additional distance to leave between the
          segment and circle. (Probably set this to the ship radius, 0.5.)
   :return: True if intersects, False otherwise
   :rtype: bool
   */
pub fn intersect_segment_circle<E: Entity, F: Entity, G: Entity>(start: &E, end: &F, circle: &G, fudge: f64) -> bool {
    let Position(start_x, start_y) = start.get_position();
    let Position(end_x, end_y) = end.get_position();
    let Position(circle_x, circle_y) = circle.get_position();
    let dx = end_x - start_x;
    let dy = end_y - start_y;

    let a = dx.powi(2) + dy.powi(2);
    let b = -2.0
        * (start_x.powi(2) - start_x * end_x - start_x * circle_x + end_x * circle_x + start_y.powi(2) - start_y * end_y
            - start_y * circle_y + end_y * circle_y);

    if a == 0.0 {
        // Start and end are the same point.
        return start.distance_to(circle) <= circle.get_radius() + fudge;
    }

    let &t = [-b / (2.0 * a), 1.0]
        .iter()
        .min_by(|x, y| x.partial_cmp(y).unwrap())
        .unwrap();
    if t < 0.0 {
        return false;
    }

    let closest_x = start_x + dx * t;
    let closest_y = start_y + dy * t;
    let closest_distance = Position(closest_x, closest_y).distance_to(circle);

    return closest_distance <= circle.get_radius() + fudge;
}
