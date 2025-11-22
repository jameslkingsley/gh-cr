use anyhow::Result;

pub struct Threads {}

impl Threads {
    // #[control(key = 'r', label = "reply")]
    pub async fn reply(&self) -> Result<()> {
        Ok(())
    }
}
