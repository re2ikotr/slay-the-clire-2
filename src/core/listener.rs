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
pub enum CardListenerScope {
    All,
    SourceOnly,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelatedListenerScope {
    pub creatures: Vec<CreatureId>,
    pub source: Option<Source>,
    pub include_player_inventory: bool,
    pub cards: CardListenerScope,
    pub include_modifiers: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListenerScope {
    Combat,
    Related(RelatedListenerScope),
}

impl ListenerScope {
    pub fn related(
        creatures: impl IntoIterator<Item = CreatureId>,
        source: Option<Source>,
    ) -> Self {
        Self::Related(RelatedListenerScope {
            creatures: dedupe_creatures(creatures),
            source,
            include_player_inventory: true,
            cards: CardListenerScope::All,
            include_modifiers: true,
        })
    }

    pub fn related_with_cards(
        creatures: impl IntoIterator<Item = CreatureId>,
        source: Option<Source>,
        cards: CardListenerScope,
    ) -> Self {
        Self::Related(RelatedListenerScope {
            creatures: dedupe_creatures(creatures),
            source,
            include_player_inventory: true,
            cards,
            include_modifiers: true,
        })
    }
}

pub fn collect_combat_listeners(state: &GameState, scope: ListenerScope) -> Vec<ListenerRef> {
    let Some(combat) = state.combat() else {
        return Vec::new();
    };

    let mut out = Vec::new();

    for creature in &combat.creatures {
        out.extend(
            creature
                .powers
                .iter()
                .copied()
                .map(ListenerRef::Power)
                .filter(|listener| listener_is_in_scope(combat, *listener, &scope)),
        );
    }

    for creature in &combat.creatures {
        let listener = ListenerRef::Monster(creature.id);
        if creature.side == Side::Monsters && listener_is_in_scope(combat, listener, &scope) {
            out.push(ListenerRef::Monster(creature.id));
        }
    }

    out.extend(
        combat
            .player
            .relics
            .iter()
            .copied()
            .map(ListenerRef::Relic)
            .filter(|listener| listener_is_in_scope(combat, *listener, &scope)),
    );
    out.extend(
        combat
            .player
            .potions
            .iter()
            .copied()
            .map(ListenerRef::Potion)
            .filter(|listener| listener_is_in_scope(combat, *listener, &scope)),
    );
    out.extend(
        combat
            .player
            .orb_queue
            .orbs
            .iter()
            .copied()
            .map(ListenerRef::Orb)
            .filter(|listener| listener_is_in_scope(combat, *listener, &scope)),
    );

    let cards = combat.player.piles.all_cards_in_pile_order();
    out.extend(
        cards
            .iter()
            .copied()
            .map(ListenerRef::Card)
            .filter(|listener| listener_is_in_scope(combat, *listener, &scope)),
    );
    out.extend(
        cards
            .iter()
            .copied()
            .map(ListenerRef::Affliction)
            .filter(|listener| listener_is_in_scope(combat, *listener, &scope)),
    );
    out.extend(
        cards
            .into_iter()
            .map(ListenerRef::Enchantment)
            .filter(|listener| listener_is_in_scope(combat, *listener, &scope)),
    );

    out.extend(
        combat
            .modifiers
            .iter()
            .copied()
            .map(ListenerRef::Modifier)
            .filter(|listener| listener_is_in_scope(combat, *listener, &scope)),
    );

    // Keep monsters in creature order above. This guard makes the intended
    // player/monster split visible until more listener categories are added.
    debug_assert!(combat
        .creatures
        .iter()
        .any(|creature| creature.side == Side::Player));

    out
}

fn listener_is_in_scope(
    combat: &crate::core::state::CombatState,
    listener: ListenerRef,
    scope: &ListenerScope,
) -> bool {
    let ListenerScope::Related(scope) = scope else {
        return true;
    };

    if source_matches_listener(scope.source, listener) {
        return true;
    }

    match listener {
        ListenerRef::Power(id) => combat
            .powers
            .get(&id)
            .map(|power| scope.creatures.contains(&power.owner))
            .unwrap_or(false),
        ListenerRef::Monster(id) => scope.creatures.contains(&id),
        ListenerRef::Relic(_) | ListenerRef::Potion(_) => scope.include_player_inventory,
        ListenerRef::Orb(id) => {
            scope.include_player_inventory
                && combat
                    .orbs
                    .get(&id)
                    .map(|orb| {
                        orb.owner == combat.player.id
                            && scope.creatures.contains(&combat.player.creature)
                    })
                    .unwrap_or(false)
        }
        ListenerRef::Card(id) | ListenerRef::Affliction(id) | ListenerRef::Enchantment(id) => {
            match scope.cards {
                CardListenerScope::All => true,
                CardListenerScope::SourceOnly => {
                    matches!(scope.source, Some(Source::Card(card)) if card == id)
                }
                CardListenerScope::None => false,
            }
        }
        ListenerRef::Modifier(_) => scope.include_modifiers,
    }
}

fn source_matches_listener(source: Option<Source>, listener: ListenerRef) -> bool {
    match (source, listener) {
        (Some(Source::Power(source)), ListenerRef::Power(listener)) => source == listener,
        (Some(Source::Relic(source)), ListenerRef::Relic(listener)) => source == listener,
        (Some(Source::Potion(source)), ListenerRef::Potion(listener)) => source == listener,
        (Some(Source::Creature(source)), ListenerRef::Monster(listener)) => source == listener,
        _ => false,
    }
}

fn dedupe_creatures(creatures: impl IntoIterator<Item = CreatureId>) -> Vec<CreatureId> {
    let mut out = Vec::new();
    for creature in creatures {
        if !out.contains(&creature) {
            out.push(creature);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::core::effect::Source;
    use crate::core::ids::CreatureId;
    use crate::core::ids::{
        ModifierInstanceId, PotionId, PotionInstanceId, PowerId, PowerInstanceId, RelicId,
        RelicInstanceId,
    };
    use crate::core::state::{
        Creature, GameState, PotionInstance, PowerInstance, RelicInstance, Side,
    };

    use super::{collect_combat_listeners, CardListenerScope, ListenerRef, ListenerScope};

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
                amount: 1,
                counters: Default::default(),
            },
        );
        combat.powers.insert(
            enemy_power,
            PowerInstance {
                id: enemy_power,
                def: PowerId::new("enemy_power"),
                owner: enemy,
                amount: 1,
                counters: Default::default(),
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

    #[test]
    fn related_listener_scope_keeps_order_while_filtering_creatures() {
        let mut state = GameState::demo_combat(22);
        let player_creature = state.player_creature_id().unwrap();
        let enemy = state.combat().unwrap().monster_ids()[0];
        let other_enemy = CreatureId::new(3);
        let card = state.combat().unwrap().player.piles.hand[0];

        let player_power = PowerInstanceId::new(10);
        let enemy_power = PowerInstanceId::new(11);
        let other_power = PowerInstanceId::new(12);
        let relic = RelicInstanceId::new(20);
        let potion = PotionInstanceId::new(30);
        let modifier = ModifierInstanceId::new(40);

        let combat = state.combat_mut().unwrap();
        combat
            .creatures
            .push(Creature::new(other_enemy, Side::Monsters, 10));
        for (power, owner, def) in [
            (player_power, player_creature, "player_power"),
            (enemy_power, enemy, "enemy_power"),
            (other_power, other_enemy, "other_power"),
        ] {
            combat.powers.insert(
                power,
                PowerInstance {
                    id: power,
                    def: PowerId::new(def),
                    owner,
                    amount: 1,
                    counters: Default::default(),
                },
            );
            combat
                .creatures
                .iter_mut()
                .find(|creature| creature.id == owner)
                .unwrap()
                .powers
                .push(power);
        }
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

        let listeners = collect_combat_listeners(
            &state,
            ListenerScope::related([player_creature, enemy], Some(Source::Card(card))),
        );

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

    #[test]
    fn source_only_card_scope_limits_card_listeners_to_the_source_card() {
        let state = GameState::demo_combat(23);
        let enemy = state.combat().unwrap().monster_ids()[0];
        let card = state.combat().unwrap().player.piles.hand[0];

        let listeners = collect_combat_listeners(
            &state,
            ListenerScope::related_with_cards(
                [enemy],
                Some(Source::Card(card)),
                CardListenerScope::SourceOnly,
            ),
        );

        assert!(listeners.contains(&ListenerRef::Card(card)));
        assert!(listeners.contains(&ListenerRef::Affliction(card)));
        assert!(listeners.contains(&ListenerRef::Enchantment(card)));
    }
}
