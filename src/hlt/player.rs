use hlt::entity::Ship;
use hlt::parse::Decodable;

#[derive(PartialEq, Debug)]
pub struct Player {
    pub id: i32,
    pub ships: Vec<Ship>,
}

impl Player {
    pub fn all_ships(&self) -> &Vec<Ship> {
        &self.ships
    }

    pub fn owns_ship(&self, ship_id: i32) -> bool {
        self.ships.iter().any(|s| s.id == ship_id)
    }
}

impl Decodable for Player {
    fn parse<'a, I>(tokens: &mut I) -> Player
    where
        I: Iterator<Item = &'a str>,
    {
        let id = i32::parse(tokens);
        let ships = Vec::parse(tokens);

        return Player { id, ships };
    }
}
