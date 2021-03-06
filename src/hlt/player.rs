use hlt::entity::Ship;
use hlt::parse::Decodable;

#[derive(PartialEq, Debug)]
pub struct Player {
    pub id: i32,
    pub ships: Vec<Ship>,
    pub strength: f64,
}

impl Player {
    pub fn all_ships(&self) -> &Vec<Ship> {
        &self.ships
    }
}

impl Decodable for Player {
    fn parse<'a, I>(tokens: &mut I) -> Player
    where
        I: Iterator<Item = &'a str>,
    {
        let id = i32::parse(tokens);
        let ships = Vec::parse(tokens);
        let strength = 0.0;

        return Player {
            id,
            ships,
            strength,
        };
    }
}
