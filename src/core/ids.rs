macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(value: u32) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u32 {
                self.0
            }
        }
    };
}

id_type!(PlayerId);
id_type!(CreatureId);
id_type!(CardInstanceId);
id_type!(PowerInstanceId);
id_type!(RelicInstanceId);
id_type!(PotionInstanceId);
id_type!(OrbInstanceId);
id_type!(ModifierInstanceId);
id_type!(ChoiceId);
id_type!(CombatId);

macro_rules! static_id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(&'static str);

        impl $name {
            pub const fn new(value: &'static str) -> Self {
                Self(value)
            }

            pub const fn as_str(self) -> &'static str {
                self.0
            }
        }
    };
}

static_id_type!(CardId);
static_id_type!(PowerId);
static_id_type!(RelicId);
static_id_type!(PotionId);
static_id_type!(MonsterId);
static_id_type!(OrbId);
static_id_type!(LocKey);
