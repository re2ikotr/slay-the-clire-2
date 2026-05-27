use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::core::ids::{
    CardId, CardInstanceId, CreatureId, OrbId, OrbInstanceId, PlayerId, PowerId, PowerInstanceId,
};
use crate::core::state::{
    decimal_to_i32_trunc, CardCosts, CardCounter, CardFlags, CardInstance, GameState, OrbInstance,
    PileId, PileKind, PowerInstance, ResourceKind, Side, StateError, TemporaryCardCosts,
    MAX_CARDS_IN_HAND, MAX_ORB_SLOTS,
};

impl GameState {
    pub fn combat(&self) -> Option<&crate::core::state::CombatState> {
        self.combat.as_ref()
    }

    pub fn combat_mut(&mut self) -> Option<&mut crate::core::state::CombatState> {
        self.combat.as_mut()
    }

    pub fn player_id(&self) -> Option<PlayerId> {
        self.combat.as_ref().map(|combat| combat.player.id)
    }

    pub fn player_creature_id(&self) -> Option<CreatureId> {
        self.combat.as_ref().map(|combat| combat.player.creature)
    }

    pub fn creature(&self, id: CreatureId) -> Option<&crate::core::state::Creature> {
        self.combat
            .as_ref()?
            .creatures
            .iter()
            .find(|creature| creature.id == id)
    }

    pub fn creature_mut(&mut self, id: CreatureId) -> Option<&mut crate::core::state::Creature> {
        self.combat
            .as_mut()?
            .creatures
            .iter_mut()
            .find(|creature| creature.id == id)
    }

    pub fn card(&self, id: CardInstanceId) -> Option<&CardInstance> {
        self.combat.as_ref()?.cards.get(&id)
    }

    pub fn card_mut(&mut self, id: CardInstanceId) -> Option<&mut CardInstance> {
        self.combat.as_mut()?.cards.get_mut(&id)
    }

    pub fn orb(&self, id: OrbInstanceId) -> Option<&OrbInstance> {
        self.combat.as_ref()?.orbs.get(&id)
    }

    pub fn card_is_in_pile(&self, id: CardInstanceId, pile: PileKind) -> bool {
        self.card(id)
            .map(|card| card.pile.kind == pile)
            .unwrap_or(false)
    }

    pub fn alive_monster_ids(&self) -> Vec<CreatureId> {
        self.combat
            .as_ref()
            .map(|combat| {
                combat
                    .creatures
                    .iter()
                    .filter(|creature| creature.side == Side::Monsters && creature.is_hittable())
                    .map(|creature| creature.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn power_amount(&self, owner: CreatureId, power: PowerId) -> i32 {
        self.combat
            .as_ref()
            .and_then(|combat| {
                self.creature(owner).map(|creature| {
                    creature
                        .powers
                        .iter()
                        .filter_map(|id| combat.powers.get(id))
                        .filter(|instance| instance.def == power)
                        .map(|instance| instance.amount)
                        .sum()
                })
            })
            .unwrap_or(0)
    }

    pub fn has_power(&self, owner: CreatureId, power: PowerId) -> bool {
        self.power_amount(owner, power) != 0
    }

    pub fn resource_amount(
        &self,
        player: PlayerId,
        resource: ResourceKind,
    ) -> Result<i32, StateError> {
        let combat = self.combat.as_ref().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }
        Ok(match resource {
            ResourceKind::Energy => combat.player.energy,
            ResourceKind::Stars => combat.player.stars,
        })
    }

    pub fn orb_owner_creature(&self, orb: OrbInstanceId) -> Option<CreatureId> {
        let owner = self.orb(orb)?.owner;
        self.combat.as_ref().and_then(|combat| {
            if combat.player.id == owner {
                Some(combat.player.creature)
            } else {
                None
            }
        })
    }

    pub fn spend_resource(
        &mut self,
        player: PlayerId,
        resource: ResourceKind,
        amount: i32,
    ) -> Result<(), StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }
        if amount < 0 {
            return Err(StateError::InvalidResourceAmount { resource, amount });
        }
        let available = match resource {
            ResourceKind::Energy => combat.player.energy,
            ResourceKind::Stars => combat.player.stars,
        };
        if available < amount {
            return Err(StateError::NotEnoughResource {
                player,
                resource,
                available,
                required: amount,
            });
        }
        match resource {
            ResourceKind::Energy => combat.player.energy -= amount,
            ResourceKind::Stars => combat.player.stars -= amount,
        }
        Ok(())
    }

    pub fn gain_resource(
        &mut self,
        player: PlayerId,
        resource: ResourceKind,
        amount: i32,
    ) -> Result<(), StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }
        if amount < 0 {
            return Err(StateError::InvalidResourceAmount { resource, amount });
        }
        match resource {
            ResourceKind::Energy => combat.player.energy += amount,
            ResourceKind::Stars => combat.player.stars += amount,
        }
        Ok(())
    }

    pub fn gain_block(&mut self, target: CreatureId, amount: Decimal) -> Result<i32, StateError> {
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let before = creature.block;
        let gained = decimal_to_i32_trunc(amount);
        creature.block = creature.block.saturating_add(gained).min(999_999_999);
        Ok(creature.block - before)
    }

    pub fn lose_block(&mut self, target: CreatureId, amount: i32) -> Result<i32, StateError> {
        let block_loss = amount.max(0);
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let before = creature.block;
        creature.block = creature.block.saturating_sub(block_loss);
        Ok(before - creature.block)
    }

    pub fn lose_hp(&mut self, target: CreatureId, amount: Decimal) -> Result<i32, StateError> {
        let hp_loss = decimal_to_i32_trunc(amount).max(0);
        let player = self.player_creature_id();
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let before = creature.hp;
        creature.hp = creature.hp.saturating_sub(hp_loss).max(0);
        let actual = before - creature.hp;
        if actual > 0 && Some(target) == player {
            self.record_player_hp_loss(actual);
        }
        Ok(actual)
    }

    pub fn heal(&mut self, target: CreatureId, amount: Decimal) -> Result<i32, StateError> {
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let before = creature.hp;
        let amount = decimal_to_i32_trunc(amount).max(0);
        creature.hp = creature.hp.saturating_add(amount).min(creature.max_hp);
        Ok(creature.hp - before)
    }

    pub fn gain_max_hp(&mut self, target: CreatureId, amount: i32) -> Result<i32, StateError> {
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let amount = amount.max(0);
        creature.max_hp = creature.max_hp.saturating_add(amount);
        creature.hp = creature.hp.saturating_add(amount).min(creature.max_hp);
        Ok(amount)
    }

    pub fn move_card(
        &mut self,
        card: CardInstanceId,
        to: PileId,
    ) -> Result<Option<PileKind>, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        let (owner, current_pile) = combat
            .cards
            .get(&card)
            .map(|card| (card.owner, card.pile))
            .ok_or(StateError::UnknownCard(card))?;

        if owner != combat.player.id || to.owner != owner {
            return Err(StateError::UnknownPlayer(owner));
        }

        if !combat.player.piles.remove_from(current_pile.kind, card) {
            return Err(StateError::CardMissingFromPile {
                card,
                pile: current_pile,
            });
        }

        let from = Some(current_pile.kind);
        combat.player.piles.push(to.kind, card);

        if let Some(card_state) = combat.cards.get_mut(&card) {
            card_state.pile = to;
            if !matches!(to.kind, PileKind::Hand) {
                card_state.clear_turn_limited_state();
            }
        }

        Ok(from)
    }

    pub fn add_generated_card(
        &mut self,
        player: PlayerId,
        def: CardId,
        to: PileId,
        upgraded: bool,
        costs: CardCosts,
        temporary: bool,
        zero_cost_this_turn: bool,
    ) -> Result<CardInstanceId, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player || to.owner != player {
            return Err(StateError::UnknownPlayer(player));
        }

        let id = combat.alloc_card_instance_id();
        combat.cards.insert(
            id,
            CardInstance {
                id,
                def,
                owner: player,
                upgraded,
                costs,
                temp_costs: if zero_cost_this_turn {
                    TemporaryCardCosts {
                        energy: Some(crate::core::state::CardCost::Fixed(0)),
                        stars: None,
                    }
                } else {
                    TemporaryCardCosts::default()
                },
                pile: to,
                flags: CardFlags {
                    temporary,
                    zero_cost_this_turn,
                    ..CardFlags::default()
                },
                counters: BTreeMap::new(),
            },
        );
        combat.player.piles.push(to.kind, id);
        Ok(id)
    }

    pub fn exhaust_card(&mut self, card: CardInstanceId) -> Result<Option<PileKind>, StateError> {
        let owner = self.card(card).ok_or(StateError::UnknownCard(card))?.owner;
        self.move_card(card, PileId::player(owner, PileKind::Exhaust))
    }

    pub fn upgrade_card(&mut self, card: CardInstanceId) -> Result<bool, StateError> {
        let card = self.card_mut(card).ok_or(StateError::UnknownCard(card))?;
        if card.upgraded {
            return Ok(false);
        }
        card.upgraded = true;
        Ok(true)
    }

    pub fn add_card_counter(
        &mut self,
        card: CardInstanceId,
        counter: CardCounter,
        amount: i32,
    ) -> Result<i32, StateError> {
        let card = self.card_mut(card).ok_or(StateError::UnknownCard(card))?;
        let value = card.counters.entry(counter).or_insert(0);
        *value += amount;
        Ok(*value)
    }

    pub fn add_orb_slots(&mut self, player: PlayerId, amount: u8) -> Result<u8, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }

        let before = combat.player.orb_queue.slots;
        combat.player.orb_queue.slots = combat
            .player
            .orb_queue
            .slots
            .saturating_add(amount)
            .min(MAX_ORB_SLOTS);
        Ok(combat.player.orb_queue.slots - before)
    }

    pub fn remove_orb_slots(&mut self, player: PlayerId, amount: u8) -> Result<u8, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }

        let before = combat.player.orb_queue.slots;
        combat.player.orb_queue.slots = combat.player.orb_queue.slots.saturating_sub(amount);
        while combat.player.orb_queue.orbs.len() > usize::from(combat.player.orb_queue.slots) {
            let Some(orb) = combat.player.orb_queue.orbs.pop() else {
                break;
            };
            combat.orbs.remove(&orb);
        }
        Ok(before - combat.player.orb_queue.slots)
    }

    pub fn channel_orb(
        &mut self,
        player: PlayerId,
        def: OrbId,
    ) -> Result<OrbInstanceId, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }

        if !combat.player.orb_queue.has_room() {
            return Err(StateError::NoOrbSlot(player));
        }

        let id = combat.alloc_orb_instance_id();
        combat.orbs.insert(
            id,
            OrbInstance {
                id,
                def,
                owner: player,
            },
        );
        combat.player.orb_queue.orbs.push(id);
        Ok(id)
    }

    pub fn remove_orb(&mut self, orb: OrbInstanceId) -> Result<OrbInstance, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        let instance = combat
            .orbs
            .remove(&orb)
            .ok_or(StateError::UnknownOrb(orb))?;
        if combat.player.id == instance.owner {
            combat
                .player
                .orb_queue
                .orbs
                .retain(|existing| *existing != orb);
        }
        Ok(instance)
    }

    pub fn shuffle_discard_into_draw_if_needed(
        &mut self,
        player: PlayerId,
    ) -> Result<Option<Vec<CardInstanceId>>, StateError> {
        let rng = &mut self.rng.shuffle;
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }

        if !combat.player.piles.draw.is_empty() || combat.player.piles.discard.is_empty() {
            return Ok(None);
        }

        let mut cards = std::mem::take(&mut combat.player.piles.discard);
        rng.shuffle(&mut cards);
        for card in &cards {
            if let Some(card_state) = combat.cards.get_mut(card) {
                card_state.pile = PileId::player(player, PileKind::Draw);
            }
        }
        combat.player.piles.draw = cards.clone();
        Ok(Some(cards))
    }

    pub fn draw_one_card(
        &mut self,
        player: PlayerId,
    ) -> Result<Option<CardInstanceId>, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }
        if combat.player.piles.hand.len() >= MAX_CARDS_IN_HAND {
            return Ok(None);
        }

        let Some(card) = combat.player.piles.draw.pop() else {
            return Ok(None);
        };
        combat.player.piles.hand.push(card);
        if let Some(card_state) = combat.cards.get_mut(&card) {
            card_state.pile = PileId::player(player, PileKind::Hand);
        }
        Ok(Some(card))
    }

    pub fn discard_hand(&mut self, player: PlayerId) -> Result<Vec<CardInstanceId>, StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        if combat.player.id != player {
            return Err(StateError::UnknownPlayer(player));
        }

        let cards = std::mem::take(&mut combat.player.piles.hand);
        for card in &cards {
            combat.player.piles.discard.push(*card);
            if let Some(card_state) = combat.cards.get_mut(card) {
                card_state.pile = PileId::player(player, PileKind::Discard);
            }
        }

        Ok(cards)
    }

    pub fn clear_block(&mut self, target: CreatureId) -> Result<i32, StateError> {
        let creature = self
            .creature_mut(target)
            .ok_or(StateError::UnknownCreature(target))?;
        let amount = creature.block;
        creature.block = 0;
        Ok(amount)
    }

    pub fn increment_turns_taken(&mut self, creature: CreatureId) -> Result<(), StateError> {
        let creature = self
            .creature_mut(creature)
            .ok_or(StateError::UnknownCreature(creature))?;
        creature.turns_taken += 1;
        Ok(())
    }

    pub fn apply_power(
        &mut self,
        target: CreatureId,
        power: PowerId,
        amount: Decimal,
    ) -> Result<(PowerInstanceId, i32), StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        let amount = decimal_to_i32_trunc(amount);
        let target_index = combat
            .creatures
            .iter()
            .position(|creature| creature.id == target)
            .ok_or(StateError::UnknownCreature(target))?;

        let existing = combat.creatures[target_index]
            .powers
            .iter()
            .copied()
            .find(|power_id| {
                combat
                    .powers
                    .get(power_id)
                    .map(|instance| instance.def == power)
                    .unwrap_or(false)
            });

        if let Some(existing) = existing {
            if let Some(instance) = combat.powers.get_mut(&existing) {
                instance.amount += amount;
            }
            return Ok((existing, amount));
        }

        let id = combat.alloc_power_instance_id();
        combat.powers.insert(
            id,
            PowerInstance {
                id,
                def: power,
                owner: target,
                amount,
            },
        );
        combat.creatures[target_index].powers.push(id);
        Ok((id, amount))
    }

    pub fn add_power_amount(
        &mut self,
        power: PowerInstanceId,
        amount: Decimal,
    ) -> Result<(CreatureId, PowerId, i32), StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        let instance = combat
            .powers
            .get_mut(&power)
            .ok_or(StateError::UnknownPower(power))?;
        let delta = decimal_to_i32_trunc(amount);
        instance.amount += delta;
        Ok((instance.owner, instance.def, delta))
    }

    pub fn remove_power(&mut self, power: PowerInstanceId) -> Result<(), StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        let Some(instance) = combat.powers.remove(&power) else {
            return Ok(());
        };
        if let Some(creature) = combat
            .creatures
            .iter_mut()
            .find(|creature| creature.id == instance.owner)
        {
            creature.powers.retain(|existing| *existing != power);
        }
        Ok(())
    }

    pub fn death_candidates(&self) -> Vec<CreatureId> {
        self.combat
            .as_ref()
            .map(|combat| {
                combat
                    .creatures
                    .iter()
                    .filter(|creature| creature.alive && creature.hp <= 0)
                    .map(|creature| creature.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn mark_dead(&mut self, creature: CreatureId) -> Result<(), StateError> {
        let target = self
            .creature_mut(creature)
            .ok_or(StateError::UnknownCreature(creature))?;
        target.alive = false;
        Ok(())
    }

    pub fn set_phase(&mut self, phase: crate::core::state::CombatPhase) -> Result<(), StateError> {
        let combat = self.combat.as_mut().ok_or(StateError::CombatNotActive)?;
        combat.phase = phase;
        Ok(())
    }

    pub fn reset_turn_stats(&mut self) {
        if let Some(combat) = self.combat.as_mut() {
            combat.turn_stats = crate::core::state::CombatTurnStats::default();
        }
    }

    pub fn record_attack_played(&mut self) {
        if let Some(combat) = self.combat.as_mut() {
            combat.turn_stats.attacks_played += 1;
        }
    }

    pub fn record_card_played(&mut self) {
        if let Some(combat) = self.combat.as_mut() {
            combat.turn_stats.cards_played += 1;
        }
    }

    pub fn record_card_exhausted(&mut self) {
        if let Some(combat) = self.combat.as_mut() {
            combat.turn_stats.cards_exhausted += 1;
        }
    }

    pub fn record_card_block_gained(&mut self) {
        if let Some(combat) = self.combat.as_mut() {
            combat.turn_stats.card_block_gains += 1;
        }
    }

    pub fn record_player_hp_loss(&mut self, amount: i32) {
        if amount <= 0 {
            return;
        }
        if let Some(combat) = self.combat.as_mut() {
            combat.turn_stats.hp_lost_by_player += amount;
            combat.combat_stats.hp_loss_events_by_player += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::core::ids::CardInstanceId;
    use crate::core::state::{GameState, PileId, PileKind, StateError};

    #[test]
    fn move_card_requires_card_to_be_in_recorded_source_pile() {
        let mut state = GameState::demo_combat(1);
        let player = state.player_id().unwrap();
        let card = CardInstanceId::new(1);
        let hand = PileId::player(player, PileKind::Hand);

        let combat = state.combat_mut().unwrap();
        combat.player.piles.hand.clear();
        combat.player.piles.discard.push(card);

        let result = state.move_card(card, PileId::player(player, PileKind::Draw));

        assert_eq!(
            result,
            Err(StateError::CardMissingFromPile { card, pile: hand })
        );
        let combat = state.combat().unwrap();
        assert!(combat.player.piles.draw.is_empty());
        assert_eq!(combat.player.piles.discard, vec![card]);
        assert_eq!(combat.cards.get(&card).unwrap().pile, hand);
    }
}
