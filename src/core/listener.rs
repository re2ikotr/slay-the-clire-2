use crate::core::effect::Source;
use crate::core::ids::{
    CardInstanceId, CreatureId, ModifierInstanceId, OrbInstanceId, PotionInstanceId,
    PowerInstanceId, RelicInstanceId,
};
use crate::core::state::{GameState, Side};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ListenerRef {
    Power(PowerInstanceId),
    Monster(CreatureId),
    Relic(RelicInstanceId),
    Potion(PotionInstanceId),
    Orb(OrbInstanceId),
    Card(CardInstanceId),
    Affliction(CardInstanceId),
    Enchantment(CardInstanceId),
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
        out.extend(creature.powers.iter().copied().map(ListenerRef::Power));
    }

    for creature in &combat.creatures {
        if creature.side == Side::Monsters {
            out.push(ListenerRef::Monster(creature.id));
        }
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

    let cards = combat.player.piles.all_cards_in_pile_order();
    out.extend(cards.iter().copied().map(ListenerRef::Card));
    out.extend(cards.iter().copied().map(ListenerRef::Affliction));
    out.extend(cards.into_iter().map(ListenerRef::Enchantment));

    out.extend(combat.modifiers.iter().copied().map(ListenerRef::Modifier));

    // Keep monsters in creature order above. This guard makes the intended
    // player/monster split visible until more listener categories are added.
    debug_assert!(combat
        .creatures
        .iter()
        .any(|creature| creature.side == Side::Player));

    out
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::core::ids::{
        ModifierInstanceId, PotionId, PotionInstanceId, PowerId, PowerInstanceId, RelicId,
        RelicInstanceId,
    };
    use crate::core::state::{GameState, PotionInstance, PowerInstance, RelicInstance};

    use super::{collect_combat_listeners, ListenerRef, ListenerScope};

    #[test]
    fn combat_listener_order_is_explicit() {
        let mut state = GameState::demo_combat(21);
        let player_creature = state.player_creature_id().unwrap();
        let enemy = state.combat().unwrap().monster_ids()[0];
        let card = state.combat().unwrap().player.piles.hand[0];

        let player_power = PowerInstanceId::new(10);
        let enemy_power = PowerInstanceId::new(11);
        let relic = RelicInstanceId::new(20);
        let potion = PotionInstanceId::new(30);
        let modifier = ModifierInstanceId::new(40);

        let combat = state.combat_mut().unwrap();
        combat.powers.insert(
            player_power,
            PowerInstance {
                id: player_power,
                def: PowerId::new("player_power"),
                owner: player_creature,
                amount: Decimal::from(1),
            },
        );
        combat.powers.insert(
            enemy_power,
            PowerInstance {
                id: enemy_power,
                def: PowerId::new("enemy_power"),
                owner: enemy,
                amount: Decimal::from(1),
            },
        );
        combat
            .creatures
            .iter_mut()
            .find(|creature| creature.id == player_creature)
            .unwrap()
            .powers
            .push(player_power);
        combat
            .creatures
            .iter_mut()
            .find(|creature| creature.id == enemy)
            .unwrap()
            .powers
            .push(enemy_power);
        combat.relics.insert(
            relic,
            RelicInstance {
                id: relic,
                def: RelicId::new("test_relic"),
            },
        );
        combat.player.relics.push(relic);
        combat.potions.insert(
            potion,
            PotionInstance {
                id: potion,
                def: PotionId::new("test_potion"),
            },
        );
        combat.player.potions.push(potion);
        combat.modifiers.push(modifier);

        let listeners = collect_combat_listeners(&state, ListenerScope::Combat);

        assert_eq!(
            listeners,
            vec![
                ListenerRef::Power(player_power),
                ListenerRef::Power(enemy_power),
                ListenerRef::Monster(enemy),
                ListenerRef::Relic(relic),
                ListenerRef::Potion(potion),
                ListenerRef::Card(card),
                ListenerRef::Affliction(card),
                ListenerRef::Enchantment(card),
                ListenerRef::Modifier(modifier),
            ]
        );
    }
}
