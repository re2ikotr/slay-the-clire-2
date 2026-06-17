#[derive(Clone, Debug)]
pub struct RngSet {
    pub shuffle: DeterministicRng,
    pub combat_targets: DeterministicRng,
    pub monster_ai: DeterministicRng,
    pub combat_card_generation: DeterministicRng,
    pub combat_card_selection: DeterministicRng,
    pub combat_orbs: DeterministicRng,
    pub combat_potion_generation: DeterministicRng,
    pub combat_energy_costs: DeterministicRng,
    pub niche: DeterministicRng,
}

impl RngSet {
    pub fn seeded(seed: u64) -> Self {
        Self {
            shuffle: DeterministicRng::new(seed, 0x7368_7566_666c_6501),
            combat_targets: DeterministicRng::new(seed, 0x7461_7267_6574_7302),
            monster_ai: DeterministicRng::new(seed, 0x6d6f_6e61_6900_0003),
            combat_card_generation: DeterministicRng::new(seed, 0x6361_7264_6765_6e04),
            combat_card_selection: DeterministicRng::new(seed, 0x6361_7264_7365_6c05),
            combat_orbs: DeterministicRng::new(seed, 0x6f72_6273_0000_0009),
            combat_potion_generation: DeterministicRng::new(seed, 0x706f_7469_6f6e_7306),
            combat_energy_costs: DeterministicRng::new(seed, 0x656e_6572_6779_0007),
            niche: DeterministicRng::new(seed, 0x6e69_6368_6500_0008),
        }
    }
}

/// Small deterministic RNG wrapper.
///
/// This is a placeholder until the exact STS2 RNG algorithm and stream
/// derivation are matched against the C# oracle. All game code should go
/// through this boundary so the algorithm can be swapped without touching
/// rules.
#[derive(Clone, Debug)]
pub struct DeterministicRng {
    state: u64,
    counter: u64,
}

impl DeterministicRng {
    pub fn new(seed: u64, stream_salt: u64) -> Self {
        let mut rng = Self {
            state: seed ^ stream_salt,
            counter: 0,
        };
        rng.state = rng.next_splitmix();
        rng
    }

    pub fn counter(&self) -> u64 {
        self.counter
    }

    pub fn next_u64(&mut self) -> u64 {
        self.counter += 1;
        self.next_splitmix()
    }

    pub fn next_usize(&mut self, upper_exclusive: usize) -> Option<usize> {
        if upper_exclusive == 0 {
            return None;
        }
        Some((self.next_u64() as usize) % upper_exclusive)
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            if let Some(j) = self.next_usize(i + 1) {
                items.swap(i, j);
            }
        }
    }

    fn next_splitmix(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}
