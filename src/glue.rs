use contentaddressedstorage::ID;
use messages::Sha256Sum;

impl From<ID> for Sha256Sum {
    fn from(id: ID) -> Sha256Sum {
        id.id().to_string()
    }
}
