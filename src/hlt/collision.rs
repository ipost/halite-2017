
use hlt::entity::{Entity, Position};

pub fn intersect_segment_circle<E: Entity, F: Entity, G: Entity>(start: &E, end: &F, circle: &G, fudge: f64) -> bool {
    let Position(start_x, start_y) = start.get_position();
    let Position(end_x, end_y) = end.get_position();
    let Position(circle_x, circle_y) = circle.get_position();
    let dx = end_x - start_x;
    let dy = end_y - start_y;

    let a = dx.powi(2) + dy.powi(2);
    let b = -2.0 * (start_x.powi(2) - start_x*end_x - start_x*circle_x + end_x*circle_x +
              start_y.powi(2) - start_y*end_y - start_y*circle_y + end_y*circle_y);

    if a == 0.0 {
        // Start and end are the same point.
        return start.distance_to(circle) <= circle.get_radius() + fudge;
    }

    let &t = [-b / (2.0 * a), 1.0].iter().min_by(|x, y| x.partial_cmp(y).unwrap()).unwrap();
    if t < 0.0 {
        return false;
    }

    let closest_x = start_x + dx * t;
    let closest_y = start_y + dy * t;
    let closest_distance = Position(closest_x, closest_y).distance_to(circle);

    return closest_distance <= circle.get_radius() + fudge
}
/* Ships: A, B
 * P(x, t) = position of x at time t
 * D(x, y) = distance from x to y
 * mag(v) = magnitude of vector v
 * P(ship, t) = P(ship, 0) + t * ship.velocity
 * P(ship, 0) = <ship.x, ship.y>
 * D(x, y) = mag( P(x, t) - P(y, t) )
 * constraint: D(A, B) > (a.radius + b.radius) for all t
 * tcollision = D(A, B) < (a.radius + b.radius)
 *
 *
 * t = mag((<A.x, A.y> + t * A.velocity) - (<B.x, B.y> + t * B.velocity)) < (A.radius + B.radius)
 * t = mag(<A.x + t * A.vx, A.y + t * A.vy> - <B.x + t * B.vx, B.y + t * B.vy>) < (A.radius + B.radius)
 * t = mag(<A.x + t * A.vx - B.x - t * B.vx, A.y + t * A.vy - B.y - t * B.vy>) < (A.radius + B.radius)
 * t = mag(<A.x - B.x + t * (A.vx - B.vx), A.y - B.y + t * (A.vy - B.vy)>) < (A.radius + B.radius)
 * t = sqrt((A.x - B.x + t * (A.vx - B.vx))^2 + (A.y - B.y + t * (A.vy - B.vy))^2) < (A.radius + B.radius)
 *
 * https://www.wolframalpha.com/input/?i=sqrt((A+-+B+%2B+t+*+(N+-+M))%5E2+%2B+(C+-+D+%2B+t+*+(O+-+P))%5E2)+solve+for+t
 * t = (-sqrt(-(A O - A P - B O + B P + C M - C N - D M + D N)^2) + A M - A N - B M + B N - C O + C P + D O - D P)/(M^2 - 2 M N + N^2 + O^2 - 2 O P + P^2) and M^2 - 2 M N + N^2 + O^2 - 2 O P + P^2!=0
 *
 * t = (-sqrt(-(A.x A.vy - A.x B.vy - B.x A.vy + B.x B.vy + A.y A.vx - A.y B.vx - B.y A.vx + B.y B.vx)^2) + A.x A.vx - A.x B.vx - B.x A.vx + B.x B.vx - A.y A.vy + A.y B.vy + B.y A.vy - B.y B.vy)/(A.vx^2 - 2 A.vx B.vx + B.vx^2 + A.vy^2 - 2 A.vy B.vy + B.vy^2) and A.vx^2 - 2 A.vx B.vx + B.vx^2 + A.vy^2 - 2 A.vy B.vy + B.vy^2!=0
 *
 * t = [0..1]
 */
