#[derive(sqlx::FromRow)]
pub struct MessagePair {
    pub bot: u64,
    pub user: u64,
}

impl MessagePair {
    pub fn target_for(&self, id: u64) -> u64 {
        if self.user == id {
            self.bot
        } else {
            self.user
        }
    }
}
