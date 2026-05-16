pub mod cards;
pub mod monsters;
pub mod potions;
pub mod powers;
pub mod relics;

pub use cards::CardRegistry;
pub use monsters::MonsterRegistry;
pub use potions::PotionRegistry;
pub use powers::PowerRegistry;
pub use relics::RelicRegistry;

#[derive(Default)]
pub struct StaticRegistry {
    pub cards: CardRegistry,
    pub powers: PowerRegistry,
    pub relics: RelicRegistry,
    pub potions: PotionRegistry,
    pub monsters: MonsterRegistry,
}
