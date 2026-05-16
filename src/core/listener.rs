use crate::core::effect::Source;
use crate::core::ids::{
    CardInstanceId, CreatureId, ModifierInstanceId, PotionInstanceId, PowerInstanceId,
    RelicInstanceId,
};
use crate::core::state::{GameState, Side};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ListenerRef {
    Creature(CreatureId),
    Power(PowerInstanceId),
    Relic(RelicInstanceId),
    Potion(PotionInstanceId),
    Card(CardInstanceId),
    Modifier(ModifierInstanceId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenerScope {
    Combat,
    Creature(CreatureId),
    Source(Source),
}

pub fn collect_combat_listeners(state: &GameState, _scope: ListenerScope) -> Vec<ListenerRef> {
    let Some(combat) = state.combat() else {
        return Vec::new();
    };

    let mut out = Vec::new();

    for creature in &combat.creatures {
        out.push(ListenerRef::Creature(creature.id));
        out.extend(creature.powers.iter().copied().map(ListenerRef::Power));
    }

    out.extend(combat.player.relics.iter().copied().map(ListenerRef::Relic));
    out.extend(
        combat
            .player
            .potions
            .iter()
            .copied()
            .map(ListenerRef::Potion),
    );

    out.extend(
        combat
            .player
            .piles
            .all_cards_in_pile_order()
            .into_iter()
            .map(ListenerRef::Card),
    );

    out.extend(combat.modifiers.iter().copied().map(ListenerRef::Modifier));

    // Keep monsters in creature order above. This guard makes the intended
    // player/monster split visible until more listener categories are added.
    debug_assert!(combat
        .creatures
        .iter()
        .any(|creature| creature.side == Side::Player));

    out
}
