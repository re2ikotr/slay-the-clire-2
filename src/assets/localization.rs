use crate::content::card_text::CardTextLine;
use crate::content::cards::{CardKeyword, CardType, TargetType};
use crate::content::monsters::MonsterIntent;
use crate::core::ids::LocKey;
use crate::core::state::{CardCost, CombatPhase};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Eng,
    Zhs,
}

impl Language {
    pub fn from_code(code: &str) -> Option<Self> {
        let normalized = code.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "eng" | "en" | "en-us" | "en-gb" => Some(Self::Eng),
            "zhs" | "zh" | "zh-cn" | "zh-hans" | "cn" | "chs" => Some(Self::Zhs),
            _ => None,
        }
    }

    pub fn from_env() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .find_map(|value| Self::from_code(value.split('.').next().unwrap_or(&value)))
            .unwrap_or(Self::Eng)
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Eng => "eng",
            Self::Zhs => "zhs",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Localization {
    language: Language,
}

impl Localization {
    pub fn new(language: Language) -> Self {
        Self { language }
    }

    pub fn language(self) -> Language {
        self.language
    }

    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }

    pub fn ui(self, key: &'static str) -> &'static str {
        lookup(UI_TEXT, key, self.language).unwrap_or(key)
    }

    pub fn entity_name(self, loc_key: LocKey) -> String {
        lookup(ENTITY_NAMES, loc_key.as_str(), self.language)
            .map(str::to_string)
            .unwrap_or_else(|| fallback_label(loc_key.as_str()))
    }

    pub fn card_type(self, card_type: CardType) -> &'static str {
        match (self.language, card_type) {
            (Language::Eng, CardType::Attack) => "attack",
            (Language::Eng, CardType::Skill) => "skill",
            (Language::Eng, CardType::Power) => "power",
            (Language::Eng, CardType::Status) => "status",
            (Language::Eng, CardType::Curse) => "curse",
            (Language::Eng, CardType::Quest) => "quest",
            (Language::Zhs, CardType::Attack) => "攻击",
            (Language::Zhs, CardType::Skill) => "技能",
            (Language::Zhs, CardType::Power) => "能力",
            (Language::Zhs, CardType::Status) => "状态",
            (Language::Zhs, CardType::Curse) => "诅咒",
            (Language::Zhs, CardType::Quest) => "任务",
        }
    }

    pub fn target_type(self, target: TargetType) -> &'static str {
        match (self.language, target) {
            (Language::Eng, TargetType::None) => "none",
            (Language::Eng, TargetType::Enemy) => "enemy",
            (Language::Eng, TargetType::AllEnemies) => "all enemies",
            (Language::Eng, TargetType::RandomEnemy) => "random enemy",
            (Language::Eng, TargetType::SelfTarget) => "self",
            (Language::Eng, TargetType::AnyPlayer) => "player",
            (Language::Eng, TargetType::AnyAlly) => "ally",
            (Language::Eng, TargetType::AllAllies) => "all allies",
            (Language::Eng, TargetType::AnyCreature) => "any creature",
            (Language::Eng, TargetType::TargetedNoCreature) => "target",
            (Language::Eng, TargetType::Osty) => "osty",
            (Language::Zhs, TargetType::None) => "无",
            (Language::Zhs, TargetType::Enemy) => "敌人",
            (Language::Zhs, TargetType::AllEnemies) => "全体敌人",
            (Language::Zhs, TargetType::RandomEnemy) => "随机敌人",
            (Language::Zhs, TargetType::SelfTarget) => "自身",
            (Language::Zhs, TargetType::AnyPlayer) => "玩家",
            (Language::Zhs, TargetType::AnyAlly) => "友方",
            (Language::Zhs, TargetType::AllAllies) => "全体友方",
            (Language::Zhs, TargetType::AnyCreature) => "任意单位",
            (Language::Zhs, TargetType::TargetedNoCreature) => "目标",
            (Language::Zhs, TargetType::Osty) => "奥斯蒂",
        }
    }

    pub fn phase(self, phase: CombatPhase) -> &'static str {
        match (self.language, phase) {
            (Language::Eng, CombatPhase::CombatStart) => "combat start",
            (Language::Eng, CombatPhase::PlayerStart) => "player start",
            (Language::Eng, CombatPhase::PlayerAction) => "player action",
            (Language::Eng, CombatPhase::PlayerEnd) => "player end",
            (Language::Eng, CombatPhase::EnemyAction) => "enemy action",
            (Language::Eng, CombatPhase::EnemyEnd) => "enemy end",
            (Language::Eng, CombatPhase::Victory) => "victory",
            (Language::Eng, CombatPhase::Defeat) => "defeat",
            (Language::Zhs, CombatPhase::CombatStart) => "战斗开始",
            (Language::Zhs, CombatPhase::PlayerStart) => "玩家回合开始",
            (Language::Zhs, CombatPhase::PlayerAction) => "玩家行动",
            (Language::Zhs, CombatPhase::PlayerEnd) => "玩家回合结束",
            (Language::Zhs, CombatPhase::EnemyAction) => "敌方行动",
            (Language::Zhs, CombatPhase::EnemyEnd) => "敌方回合结束",
            (Language::Zhs, CombatPhase::Victory) => "胜利",
            (Language::Zhs, CombatPhase::Defeat) => "失败",
        }
    }

    pub fn cost(self, cost: CardCost) -> String {
        match cost {
            CardCost::None => "-".to_string(),
            CardCost::Fixed(value) => value.to_string(),
            CardCost::X => "X".to_string(),
            CardCost::Unplayable => self.ui("cost.unplayable").to_string(),
        }
    }

    pub fn card_keyword(self, keyword: CardKeyword) -> &'static str {
        match (self.language, keyword) {
            (Language::Eng, CardKeyword::Exhaust) => "Exhaust",
            (Language::Eng, CardKeyword::Innate) => "Innate",
            (Language::Eng, CardKeyword::Unplayable) => "Unplayable",
            (Language::Eng, CardKeyword::Ethereal) => "Ethereal",
            (Language::Eng, CardKeyword::Temporary) => "Temporary",
            (Language::Eng, CardKeyword::PurgeOnUse) => "Purge",
            (Language::Eng, CardKeyword::FreeThisTurn) => "Free this turn",
            (Language::Eng, CardKeyword::Retain) => "Retain",
            (Language::Eng, CardKeyword::Sly) => "Sly",
            (Language::Eng, CardKeyword::Eternal) => "Eternal",
            (Language::Zhs, CardKeyword::Exhaust) => "消耗",
            (Language::Zhs, CardKeyword::Innate) => "固有",
            (Language::Zhs, CardKeyword::Unplayable) => "不能打出",
            (Language::Zhs, CardKeyword::Ethereal) => "虚无",
            (Language::Zhs, CardKeyword::Temporary) => "临时",
            (Language::Zhs, CardKeyword::PurgeOnUse) => "使用后移除",
            (Language::Zhs, CardKeyword::FreeThisTurn) => "本回合免费",
            (Language::Zhs, CardKeyword::Retain) => "保留",
            (Language::Zhs, CardKeyword::Sly) => "机敏",
            (Language::Zhs, CardKeyword::Eternal) => "永恒",
        }
    }

    pub fn card_text_line(self, line: &CardTextLine) -> String {
        match self.language {
            Language::Eng => line.eng.clone(),
            Language::Zhs => line.zhs.clone(),
        }
    }

    pub fn intent(self, intent: MonsterIntent) -> String {
        match (self.language, intent) {
            (Language::Eng, MonsterIntent::Attack { amount }) => format!("attack {amount}"),
            (Language::Eng, MonsterIntent::AttackAndBlock { attack, block }) => {
                format!("attack {attack}, block {block}")
            }
            (Language::Eng, MonsterIntent::Buff) => "buff".to_string(),
            (Language::Eng, MonsterIntent::Block { amount }) => format!("block {amount}"),
            (Language::Eng, MonsterIntent::Debuff) => "debuff".to_string(),
            (Language::Eng, MonsterIntent::Unknown) => "unknown".to_string(),
            (Language::Zhs, MonsterIntent::Attack { amount }) => format!("攻击 {amount}"),
            (Language::Zhs, MonsterIntent::AttackAndBlock { attack, block }) => {
                format!("攻击 {attack}，格挡 {block}")
            }
            (Language::Zhs, MonsterIntent::Buff) => "强化".to_string(),
            (Language::Zhs, MonsterIntent::Block { amount }) => format!("格挡 {amount}"),
            (Language::Zhs, MonsterIntent::Debuff) => "负面效果".to_string(),
            (Language::Zhs, MonsterIntent::Unknown) => "未知".to_string(),
        }
    }

    pub fn format_unknown_command(self, input: &str) -> String {
        match self.language {
            Language::Eng => format!("unknown command: {input}"),
            Language::Zhs => format!("未知命令：{input}"),
        }
    }

    pub fn format_invalid_monster_index(self, value: &str) -> String {
        match self.language {
            Language::Eng => format!("invalid monster index: {value}"),
            Language::Zhs => format!("怪物序号无效：{value}"),
        }
    }

    pub fn format_no_card_at_hand_index(self, index: usize) -> String {
        match self.language {
            Language::Eng => format!("no card at hand index {index}"),
            Language::Zhs => format!("手牌位置 {index} 没有牌"),
        }
    }

    pub fn format_started_test_combat(self, seed: u64) -> String {
        match self.language {
            Language::Eng => format!("started test combat with seed {seed}"),
            Language::Zhs => format!("已使用种子 {seed} 开始测试战斗"),
        }
    }

    pub fn format_language_changed(self) -> String {
        match self.language {
            Language::Eng => "language set to English".to_string(),
            Language::Zhs => "语言已切换为简体中文".to_string(),
        }
    }

    pub fn format_play_card(self, label: &str) -> String {
        match self.language {
            Language::Eng => format!("play {label}"),
            Language::Zhs => format!("打出 {label}"),
        }
    }
}

#[derive(Clone, Copy)]
struct LocalizedEntry {
    key: &'static str,
    eng: &'static str,
    zhs: &'static str,
}

fn lookup(entries: &[LocalizedEntry], key: &str, language: Language) -> Option<&'static str> {
    entries
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| match language {
            Language::Eng => entry.eng,
            Language::Zhs => entry.zhs,
        })
}

fn fallback_label(key: &str) -> String {
    let stem = key.rsplit_once('.').map(|(_, value)| value).unwrap_or(key);
    let mut out = String::new();

    for (index, word) in stem.split('_').filter(|word| !word.is_empty()).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(&chars.as_str().to_ascii_lowercase());
        }
    }

    if out.is_empty() {
        key.to_string()
    } else {
        out
    }
}

const UI_TEXT: &[LocalizedEntry] = &[
    LocalizedEntry { key: "app.title", eng: "Slay the Clire 2 combat test", zhs: "Slay the Clire 2 战斗测试" },
    LocalizedEntry { key: "help.compact", eng: "Commands: <card>, <card> <monster>, e=end turn, r=restart, q=quit, ?=help, lang eng|zhs", zhs: "命令：<手牌序号>，<手牌序号> <怪物序号>，e=结束回合，r=重开，q=退出，?=帮助，lang eng|zhs" },
    LocalizedEntry { key: "help.full", eng: "Play by hand index, optionally with a monster index: 1 or 1 2. Use e, r, q, or lang eng|zhs.", zhs: "输入手牌序号出牌，可追加怪物序号，例如 1 或 1 2。可用 e、r、q，或 lang eng|zhs 切换语言。" },
    LocalizedEntry { key: "help.tui.compact", eng: "Keys: arrows target/card, 1-9 select, Enter play, p/m/g/c/d/s/x view, e/r/l/q/?", zhs: "按键：方向键目标/手牌，1-9 选择，Enter 出牌，p/m/g/c/d/s/x 查看，e/r/l/q/?" },
    LocalizedEntry { key: "help.tui.full", eng: "Left/right or number keys select a card; Up/down moves target. p/m/g/c/d/s/x opens focused panels; arrows or mouse wheel scroll there.", zhs: "左右或数字键选择手牌；上下切换目标。p/m/g/c/d/s/x 打开专注栏目；方向键或鼠标滚轮滚动。" },
    LocalizedEntry { key: "label.seed", eng: "seed", zhs: "种子" },
    LocalizedEntry { key: "label.phase", eng: "phase", zhs: "阶段" },
    LocalizedEntry { key: "label.player", eng: "Player", zhs: "玩家" },
    LocalizedEntry { key: "label.hp", eng: "HP", zhs: "生命" },
    LocalizedEntry { key: "label.block", eng: "Block", zhs: "格挡" },
    LocalizedEntry { key: "label.energy", eng: "Energy", zhs: "能量" },
    LocalizedEntry { key: "label.stars", eng: "Stars", zhs: "辉星" },
    LocalizedEntry { key: "label.hand", eng: "Hand", zhs: "手牌" },
    LocalizedEntry { key: "label.status", eng: "Status", zhs: "状态" },
    LocalizedEntry { key: "label.piles", eng: "Piles", zhs: "牌堆" },
    LocalizedEntry { key: "label.draw", eng: "draw", zhs: "抽牌堆" },
    LocalizedEntry { key: "label.discard", eng: "discard", zhs: "弃牌堆" },
    LocalizedEntry { key: "label.exhaust", eng: "exhaust", zhs: "消耗牌堆" },
    LocalizedEntry { key: "label.removed", eng: "removed", zhs: "移除" },
    LocalizedEntry { key: "label.monsters", eng: "Monsters", zhs: "怪物" },
    LocalizedEntry { key: "label.intent", eng: "Intent", zhs: "意图" },
    LocalizedEntry { key: "label.messages", eng: "Messages", zhs: "消息" },
    LocalizedEntry { key: "label.cost", eng: "cost", zhs: "费用" },
    LocalizedEntry { key: "label.target", eng: "target", zhs: "目标" },
    LocalizedEntry { key: "label.none", eng: "none", zhs: "无" },
    LocalizedEntry { key: "label.empty", eng: "empty", zhs: "空" },
    LocalizedEntry { key: "label.dead", eng: "dead", zhs: "已死亡" },
    LocalizedEntry { key: "label.unknown", eng: "unknown", zhs: "未知" },
    LocalizedEntry { key: "label.view", eng: "view", zhs: "查看" },
    LocalizedEntry { key: "label.close", eng: "close", zhs: "返回" },
    LocalizedEntry { key: "entity.player", eng: "player", zhs: "玩家" },
    LocalizedEntry { key: "entity.monster", eng: "monster", zhs: "怪物" },
    LocalizedEntry { key: "action.end_turn", eng: "end turn", zhs: "结束回合" },
    LocalizedEntry { key: "status.done", eng: "done", zhs: "完成" },
    LocalizedEntry { key: "status.choice_requested", eng: "choice requested", zhs: "需要选择" },
    LocalizedEntry { key: "status.combat_ended", eng: "combat ended", zhs: "战斗结束" },
    LocalizedEntry { key: "status.rejected", eng: "rejected", zhs: "被拒绝" },
    LocalizedEntry { key: "status.failed", eng: "failed", zhs: "失败" },
    LocalizedEntry { key: "choice.exhaust_card", eng: "Choose a card to Exhaust.", zhs: "选择1张牌消耗。" },
    LocalizedEntry { key: "unit.log_entries", eng: "log entries", zhs: "条日志" },
    LocalizedEntry { key: "error.card_index_starts_at_one", eng: "card index starts at 1", zhs: "手牌序号从 1 开始" },
    LocalizedEntry { key: "error.monster_index_starts_at_one", eng: "monster index starts at 1", zhs: "怪物序号从 1 开始" },
    LocalizedEntry { key: "error.language_usage", eng: "language must be eng or zhs", zhs: "语言必须是 eng 或 zhs" },
    LocalizedEntry { key: "error.no_active_combat", eng: "no active combat", zhs: "没有进行中的战斗" },
    LocalizedEntry { key: "error.not_player_action_phase", eng: "not in player action phase", zhs: "当前不是玩家行动阶段" },
    LocalizedEntry { key: "cost.energy", eng: "E", zhs: "能" },
    LocalizedEntry { key: "cost.stars", eng: "S", zhs: "星" },
    LocalizedEntry { key: "cost.unplayable", eng: "unplayable", zhs: "不能打出" },
    LocalizedEntry { key: "log.damage", eng: "damage", zhs: "伤害" },
    LocalizedEntry { key: "log.block", eng: "block", zhs: "格挡" },
    LocalizedEntry { key: "log.card_moved", eng: "card moved", zhs: "卡牌移动" },
    LocalizedEntry { key: "log.shuffled", eng: "shuffled", zhs: "洗牌" },
    LocalizedEntry { key: "log.cards", eng: "cards", zhs: "张牌" },
    LocalizedEntry { key: "log.creature_died", eng: "creature died", zhs: "单位死亡" },
    LocalizedEntry { key: "log.combat_result", eng: "combat result", zhs: "战斗结果" },
];

const ENTITY_NAMES: &[LocalizedEntry] = &[
    LocalizedEntry {
        key: "monster.nibbit",
        eng: "Nibbit",
        zhs: "小啃兽",
    },
    LocalizedEntry {
        key: "power.strength",
        eng: "Strength",
        zhs: "力量",
    },
    LocalizedEntry {
        key: "power.vulnerable",
        eng: "Vulnerable",
        zhs: "易伤",
    },
    LocalizedEntry {
        key: "power.weak",
        eng: "Weak",
        zhs: "虚弱",
    },
    LocalizedEntry {
        key: "power.barricade",
        eng: "Barricade",
        zhs: "壁垒",
    },
    LocalizedEntry {
        key: "power.colossus",
        eng: "Colossus",
        zhs: "巨像",
    },
    LocalizedEntry {
        key: "power.corruption",
        eng: "Corruption",
        zhs: "腐化",
    },
    LocalizedEntry {
        key: "power.cruelty",
        eng: "Cruelty",
        zhs: "残酷",
    },
    LocalizedEntry {
        key: "power.free_attack",
        eng: "Free Attack",
        zhs: "免费攻击",
    },
    LocalizedEntry {
        key: "power.no_draw",
        eng: "No Draw",
        zhs: "不可抽牌",
    },
    LocalizedEntry {
        key: "power.tank",
        eng: "Tank",
        zhs: "肉盾",
    },
    LocalizedEntry {
        key: "power.unmovable",
        eng: "Unmovable",
        zhs: "不动",
    },
    LocalizedEntry {
        key: "card.AGGRESSION",
        eng: "Aggression",
        zhs: "好勇斗狠",
    },
    LocalizedEntry {
        key: "card.ANGER",
        eng: "Anger",
        zhs: "愤怒",
    },
    LocalizedEntry {
        key: "card.ARMAMENTS",
        eng: "Armaments",
        zhs: "武装",
    },
    LocalizedEntry {
        key: "card.ASHEN_STRIKE",
        eng: "Ashen Strike",
        zhs: "灰烬打击",
    },
    LocalizedEntry {
        key: "card.BARRICADE",
        eng: "Barricade",
        zhs: "壁垒",
    },
    LocalizedEntry {
        key: "card.BASH",
        eng: "Bash",
        zhs: "痛击",
    },
    LocalizedEntry {
        key: "card.BATTLE_TRANCE",
        eng: "Battle Trance",
        zhs: "战斗专注",
    },
    LocalizedEntry {
        key: "card.BLOOD_WALL",
        eng: "Blood Wall",
        zhs: "血墙",
    },
    LocalizedEntry {
        key: "card.BLOODLETTING",
        eng: "Bloodletting",
        zhs: "放血",
    },
    LocalizedEntry {
        key: "card.BLUDGEON",
        eng: "Bludgeon",
        zhs: "重锤",
    },
    LocalizedEntry {
        key: "card.BODY_SLAM",
        eng: "Body Slam",
        zhs: "全身撞击",
    },
    LocalizedEntry {
        key: "card.BRAND",
        eng: "Brand",
        zhs: "烙印",
    },
    LocalizedEntry {
        key: "card.BREAK",
        eng: "Break",
        zhs: "破击",
    },
    LocalizedEntry {
        key: "card.BREAKTHROUGH",
        eng: "Breakthrough",
        zhs: "突破",
    },
    LocalizedEntry {
        key: "card.BULLY",
        eng: "Bully",
        zhs: "欺凌",
    },
    LocalizedEntry {
        key: "card.BURNING_PACT",
        eng: "Burning Pact",
        zhs: "燃烧契约",
    },
    LocalizedEntry {
        key: "card.CASCADE",
        eng: "Cascade",
        zhs: "倾泻",
    },
    LocalizedEntry {
        key: "card.CINDER",
        eng: "Cinder",
        zhs: "余烬",
    },
    LocalizedEntry {
        key: "card.COLOSSUS",
        eng: "Colossus",
        zhs: "巨像",
    },
    LocalizedEntry {
        key: "card.CONFLAGRATION",
        eng: "Conflagration",
        zhs: "焚烧",
    },
    LocalizedEntry {
        key: "card.CORRUPTION",
        eng: "Corruption",
        zhs: "腐化",
    },
    LocalizedEntry {
        key: "card.CRIMSON_MANTLE",
        eng: "Crimson Mantle",
        zhs: "绯红披风",
    },
    LocalizedEntry {
        key: "card.CRUELTY",
        eng: "Cruelty",
        zhs: "残酷",
    },
    LocalizedEntry {
        key: "card.DARK_EMBRACE",
        eng: "Dark Embrace",
        zhs: "黑暗之拥",
    },
    LocalizedEntry {
        key: "card.DEFEND_IRONCLAD",
        eng: "Defend",
        zhs: "防御",
    },
    LocalizedEntry {
        key: "card.DEMON_FORM",
        eng: "Demon Form",
        zhs: "恶魔形态",
    },
    LocalizedEntry {
        key: "card.DEMONIC_SHIELD",
        eng: "Demonic Shield",
        zhs: "恶魔护盾",
    },
    LocalizedEntry {
        key: "card.DISMANTLE",
        eng: "Dismantle",
        zhs: "拆卸",
    },
    LocalizedEntry {
        key: "card.DOMINATE",
        eng: "Dominate",
        zhs: "主宰",
    },
    LocalizedEntry {
        key: "card.DRUM_OF_BATTLE",
        eng: "Drum of Battle",
        zhs: "战鼓",
    },
    LocalizedEntry {
        key: "card.EVIL_EYE",
        eng: "Evil Eye",
        zhs: "邪眼",
    },
    LocalizedEntry {
        key: "card.EXPECT_A_FIGHT",
        eng: "Expect a Fight",
        zhs: "跃跃欲试",
    },
    LocalizedEntry {
        key: "card.FEED",
        eng: "Feed",
        zhs: "狂宴",
    },
    LocalizedEntry {
        key: "card.FEEL_NO_PAIN",
        eng: "Feel No Pain",
        zhs: "无惧疼痛",
    },
    LocalizedEntry {
        key: "card.FIEND_FIRE",
        eng: "Fiend Fire",
        zhs: "恶魔之焰",
    },
    LocalizedEntry {
        key: "card.FIGHT_ME",
        eng: "Fight Me!",
        zhs: "与我一战！",
    },
    LocalizedEntry {
        key: "card.FLAME_BARRIER",
        eng: "Flame Barrier",
        zhs: "火焰屏障",
    },
    LocalizedEntry {
        key: "card.FORGOTTEN_RITUAL",
        eng: "Forgotten Ritual",
        zhs: "被遗忘的仪式",
    },
    LocalizedEntry {
        key: "card.HAVOC",
        eng: "Havoc",
        zhs: "破灭",
    },
    LocalizedEntry {
        key: "card.HEADBUTT",
        eng: "Headbutt",
        zhs: "头槌",
    },
    LocalizedEntry {
        key: "card.HELLRAISER",
        eng: "Hellraiser",
        zhs: "地狱狂徒",
    },
    LocalizedEntry {
        key: "card.HEMOKINESIS",
        eng: "Hemokinesis",
        zhs: "御血术",
    },
    LocalizedEntry {
        key: "card.HOWL_FROM_BEYOND",
        eng: "Howl from Beyond",
        zhs: "彼岸咆哮",
    },
    LocalizedEntry {
        key: "card.IMPERVIOUS",
        eng: "Impervious",
        zhs: "岿然不动",
    },
    LocalizedEntry {
        key: "card.INFERNAL_BLADE",
        eng: "Infernal Blade",
        zhs: "地狱之刃",
    },
    LocalizedEntry {
        key: "card.INFERNO",
        eng: "Inferno",
        zhs: "狱火",
    },
    LocalizedEntry {
        key: "card.INFLAME",
        eng: "Inflame",
        zhs: "燃烧",
    },
    LocalizedEntry {
        key: "card.IRON_WAVE",
        eng: "Iron Wave",
        zhs: "铁斩波",
    },
    LocalizedEntry {
        key: "card.JUGGERNAUT",
        eng: "Juggernaut",
        zhs: "势不可当",
    },
    LocalizedEntry {
        key: "card.JUGGLING",
        eng: "Juggling",
        zhs: "杂耍",
    },
    LocalizedEntry {
        key: "card.MANGLE",
        eng: "Mangle",
        zhs: "凌虐",
    },
    LocalizedEntry {
        key: "card.MOLTEN_FIST",
        eng: "Molten Fist",
        zhs: "熔融之拳",
    },
    LocalizedEntry {
        key: "card.NOT_YET",
        eng: "Not Yet",
        zhs: "时候未到",
    },
    LocalizedEntry {
        key: "card.OFFERING",
        eng: "Offering",
        zhs: "祭品",
    },
    LocalizedEntry {
        key: "card.ONE_TWO_PUNCH",
        eng: "One-Two Punch",
        zhs: "连环拳",
    },
    LocalizedEntry {
        key: "card.PACTS_END",
        eng: "Pact's End",
        zhs: "契约终结",
    },
    LocalizedEntry {
        key: "card.PERFECTED_STRIKE",
        eng: "Perfected Strike",
        zhs: "完美打击",
    },
    LocalizedEntry {
        key: "card.PILLAGE",
        eng: "Pillage",
        zhs: "劫掠",
    },
    LocalizedEntry {
        key: "card.POMMEL_STRIKE",
        eng: "Pommel Strike",
        zhs: "剑柄打击",
    },
    LocalizedEntry {
        key: "card.PRIMAL_FORCE",
        eng: "Primal Force",
        zhs: "原始力量",
    },
    LocalizedEntry {
        key: "card.PYRE",
        eng: "Pyre",
        zhs: "薪火之源",
    },
    LocalizedEntry {
        key: "card.RAGE",
        eng: "Rage",
        zhs: "狂怒",
    },
    LocalizedEntry {
        key: "card.RAMPAGE",
        eng: "Rampage",
        zhs: "暴走",
    },
    LocalizedEntry {
        key: "card.RUPTURE",
        eng: "Rupture",
        zhs: "撕裂",
    },
    LocalizedEntry {
        key: "card.SECOND_WIND",
        eng: "Second Wind",
        zhs: "重振精神",
    },
    LocalizedEntry {
        key: "card.SETUP_STRIKE",
        eng: "Setup Strike",
        zhs: "预备打击",
    },
    LocalizedEntry {
        key: "card.SHRUG_IT_OFF",
        eng: "Shrug It Off",
        zhs: "耸肩无视",
    },
    LocalizedEntry {
        key: "card.SPITE",
        eng: "Spite",
        zhs: "怨恨",
    },
    LocalizedEntry {
        key: "card.STAMPEDE",
        eng: "Stampede",
        zhs: "惊逃",
    },
    LocalizedEntry {
        key: "card.STOKE",
        eng: "Stoke",
        zhs: "添柴",
    },
    LocalizedEntry {
        key: "card.STOMP",
        eng: "Stomp",
        zhs: "踩踏",
    },
    LocalizedEntry {
        key: "card.STONE_ARMOR",
        eng: "Stone Armor",
        zhs: "岩石铠甲",
    },
    LocalizedEntry {
        key: "card.STRIKE_IRONCLAD",
        eng: "Strike",
        zhs: "打击",
    },
    LocalizedEntry {
        key: "card.SWORD_BOOMERANG",
        eng: "Sword Boomerang",
        zhs: "飞剑回旋镖",
    },
    LocalizedEntry {
        key: "card.TANK",
        eng: "Tank",
        zhs: "肉盾",
    },
    LocalizedEntry {
        key: "card.TAUNT",
        eng: "Taunt",
        zhs: "挑衅",
    },
    LocalizedEntry {
        key: "card.TEAR_ASUNDER",
        eng: "Tear Asunder",
        zhs: "扯碎",
    },
    LocalizedEntry {
        key: "card.THRASH",
        eng: "Thrash",
        zhs: "痛殴",
    },
    LocalizedEntry {
        key: "card.THUNDERCLAP",
        eng: "Thunderclap",
        zhs: "闪电霹雳",
    },
    LocalizedEntry {
        key: "card.TREMBLE",
        eng: "Tremble",
        zhs: "战栗",
    },
    LocalizedEntry {
        key: "card.TRUE_GRIT",
        eng: "True Grit",
        zhs: "坚毅",
    },
    LocalizedEntry {
        key: "card.TWIN_STRIKE",
        eng: "Twin Strike",
        zhs: "双重打击",
    },
    LocalizedEntry {
        key: "card.UNMOVABLE",
        eng: "Unmovable",
        zhs: "坚定不移",
    },
    LocalizedEntry {
        key: "card.UNRELENTING",
        eng: "Unrelenting",
        zhs: "无情猛攻",
    },
    LocalizedEntry {
        key: "card.UPPERCUT",
        eng: "Uppercut",
        zhs: "上勾拳",
    },
    LocalizedEntry {
        key: "card.VICIOUS",
        eng: "Vicious",
        zhs: "凶恶",
    },
    LocalizedEntry {
        key: "card.WHIRLWIND",
        eng: "Whirlwind",
        zhs: "旋风斩",
    },
    LocalizedEntry {
        key: "card.GIANT_ROCK",
        eng: "Giant Rock",
        zhs: "巨石",
    },
];

#[cfg(test)]
mod tests {
    use super::{Language, Localization};
    use crate::core::ids::LocKey;

    #[test]
    fn resolves_current_card_names_in_both_languages() {
        let eng = Localization::new(Language::Eng);
        let zhs = Localization::new(Language::Zhs);

        assert_eq!(
            eng.entity_name(LocKey::new("card.BATTLE_TRANCE")),
            "Battle Trance"
        );
        assert_eq!(
            zhs.entity_name(LocKey::new("card.BATTLE_TRANCE")),
            "战斗专注"
        );
    }

    #[test]
    fn missing_entries_fall_back_to_stable_labels() {
        let loc = Localization::new(Language::Zhs);

        assert_eq!(
            loc.entity_name(LocKey::new("card.UNKNOWN_CARD")),
            "Unknown Card"
        );
    }
}
