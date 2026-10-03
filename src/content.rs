//! All roguelike content as data: shots, items, cards, vouchers, bosses, tags, cabinets.
//! Items are lists of rules (trigger → condition → effect), so new items are new data, not new code.

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Shot { Bumper, Sling, Inlane, Spin, Rollover, Lanes, Drop, Bank, Standup, Ramp, Orbit, Combo, Flipper, Nudge }

pub const SCORING: [Shot; 12] = [
    Shot::Bumper, Shot::Sling, Shot::Inlane, Shot::Spin, Shot::Rollover, Shot::Lanes,
    Shot::Drop, Shot::Bank, Shot::Standup, Shot::Ramp, Shot::Orbit, Shot::Combo,
];

pub struct ShotDef { pub name: &'static str, pub pts: f64, pub mult: f64, pub lp: f64, pub lm: f64 }

pub fn shot_def(s: Shot) -> ShotDef {
    use Shot::*;
    let d = |name, pts, mult, lp, lm| ShotDef { name, pts, mult, lp, lm };
    match s {
        Bumper => d("Bumper", 10.0, 0.0, 5.0, 0.0),
        Sling => d("Slingshot", 5.0, 0.0, 4.0, 0.0),
        Inlane => d("Inlane", 15.0, 0.0, 10.0, 0.5),
        Spin => d("Spinner", 2.0, 0.0, 2.0, 0.0),
        Rollover => d("Rollover", 20.0, 0.0, 10.0, 0.5),
        Lanes => d("Lane Set", 80.0, 2.0, 30.0, 1.0),
        Drop => d("Drop Target", 25.0, 0.0, 10.0, 0.5),
        Bank => d("Target Bank", 100.0, 2.0, 40.0, 1.0),
        Standup => d("Standup", 30.0, 0.5, 15.0, 0.5),
        Ramp => d("Ramp", 60.0, 1.0, 25.0, 1.0),
        Orbit => d("Orbit", 50.0, 1.0, 20.0, 1.0),
        Combo => d("Combo", 100.0, 2.0, 50.0, 1.0),
        Flipper => d("Flipper", 0.0, 0.0, 0.0, 0.0),
        Nudge => d("Nudge", 0.0, 0.0, 0.0, 0.0),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rarity { Common, Uncommon, Rare }

/// When a rule fires
#[derive(Clone, Copy, Debug)]
pub enum On {
    Hit(&'static [Shot]),
    AnyHit,
    EveryNth(Shot, u32),
    Streak(u32),
    BallStart,
    BallEnd,
    TableStart,
    TableEnd,
    Drain,
    Passive,
}

/// Extra condition checked when the trigger fires
#[derive(Clone, Copy, Debug)]
pub enum Cond {
    Always,
    Chance(u32),
    KindsAtLeast(usize),
    Multiball,
    NoMultiball,
    LastBall,
    FirstBall,
    FirstHit,
    LongBall(f64),
    FirstDrainOfTable,
}

/// Quantities that effects can scale with
#[derive(Clone, Copy, Debug)]
pub enum Per {
    One,
    Money5,
    Items,
    Items3,
    BallSeconds10,
    SpinsThisBall10,
    OtherSellValue,
    Stat,             // the item's own growing counter
}

#[derive(Clone, Copy, Debug)]
pub enum Eff {
    Pts(f64, Per),
    Mult(f64, Per),
    /// ×(base + k·per)
    XMult(f64, f64, Per),
    Cash(f64, Per),
    Grow(f64),
    Save,
    Destroy,
    ExtraBall,
    // passives
    Balls(i32),
    FlipperPower(f64),
    BumperKick(f64),
    BallSave(f64),
    InterestCap(i64),
    NoTilt,
    WildLanes,
    SpareChange(i64),
    Discount(f64),
    Gravity(f64),
    // copy another item's rules
    CopyBelow,
    CopyTop,
}

#[derive(Clone, Copy, Debug)]
pub struct Rule { pub on: On, pub cond: Cond, pub eff: Eff }

pub struct ItemDef {
    pub id: &'static str,
    pub name: &'static str,
    pub rarity: Rarity,
    pub cost: i64,
    pub desc: &'static str,
    pub rules: Vec<Rule>,
}

const fn r(on: On, cond: Cond, eff: Eff) -> Rule { Rule { on, cond, eff } }
use Cond::*;
use Eff::*;
use On::*;
use Per::*;
use Shot::*;

pub fn items() -> Vec<ItemDef> {
    let i = |id, name, rarity, cost, desc, rules: Vec<Rule>| ItemDef { id, name, rarity, cost, desc, rules };
    use Rarity::*;
    vec![
        // ---------------------------------------------------------------- common
        i("jester", "Jester", Common, 3, "[m]+4 Mult[/] at ball end", vec![r(BallEnd, Always, Mult(4.0, One))]),
        i("bumper_cars", "Bumper Cars", Common, 4, "Bumper hits give [m]+0.5 Mult[/]", vec![r(Hit(&[Bumper]), Always, Mult(0.5, One))]),
        i("sling_scholar", "Slingshot Scholar", Common, 4, "Slingshot hits give [p]+20 Points[/]", vec![r(Hit(&[Sling]), Always, Pts(20.0, One))]),
        i("lane_changer", "Lane Changer", Common, 4, "Rollovers give [m]+1 Mult[/]", vec![r(Hit(&[Rollover]), Always, Mult(1.0, One))]),
        i("spin_doctor", "Spin Doctor", Common, 5, "Every [k]8 spins[/] gives [m]+2 Mult[/]", vec![r(EveryNth(Spin, 8), Always, Mult(2.0, One))]),
        i("piggy_bank", "Piggy Bank", Common, 5, "Clearing the [k]Target Bank[/] earns [c]$2[/]", vec![r(Hit(&[Bank]), Always, Cash(2.0, One))]),
        i("ramp_rat", "Ramp Rat", Common, 5, "Ramps give [p]+40 Points[/] and [m]+1 Mult[/]", vec![r(Hit(&[Ramp]), Always, Pts(40.0, One)), r(Hit(&[Ramp]), Always, Mult(1.0, One))]),
        i("coin_slot", "Coin Slot", Common, 5, "Bumper hits: [k]1 in 8[/] to earn [c]$1[/]", vec![r(Hit(&[Bumper]), Chance(8), Cash(1.0, One))]),
        i("insurance", "Insurance", Common, 5, "[k]Ball Save[/] for the first 8 seconds of every ball", vec![r(Passive, Always, BallSave(8.0))]),
        i("orbit_oracle", "Orbit Oracle", Common, 5, "Orbits give [m]+3 Mult[/]", vec![r(Hit(&[Orbit]), Always, Mult(3.0, One))]),
        i("power_flippers", "Power Flippers", Common, 5, "Flippers swing [k]20% harder[/]. Flipper hits give [p]+5 Points[/]", vec![r(Passive, Always, FlipperPower(1.2)), r(Hit(&[Flipper]), Always, Pts(5.0, One))]),
        i("drop_zone", "Drop Zone", Common, 4, "Drop targets give [m]+1 Mult[/]", vec![r(Hit(&[Drop]), Always, Mult(1.0, One))]),
        i("tilt_whisperer", "Tilt Whisperer", Common, 4, "Nudging never [k]tilts[/]. Nudges give [p]+10 Points[/]", vec![r(Passive, Always, NoTilt), r(Hit(&[Nudge]), Always, Pts(10.0, One))]),
        i("bonus_round", "Bonus Round", Common, 4, "Earn [c]$3[/] at the end of each table", vec![r(TableEnd, Always, Cash(3.0, One))]),
        i("spare_change", "Spare Change", Common, 4, "Earn an extra [c]$1[/] per unused ball", vec![r(Passive, Always, SpareChange(1))]),
        i("standup_comic", "Standup Comic", Common, 4, "Standups give [p]+30 Points[/] and [m]+1 Mult[/]", vec![r(Hit(&[Standup]), Always, Pts(30.0, One)), r(Hit(&[Standup]), Always, Mult(1.0, One))]),
        i("toll_booth", "Toll Booth", Common, 5, "Ramps and orbits earn [c]$1[/]", vec![r(Hit(&[Ramp, Orbit]), Always, Cash(1.0, One))]),
        i("late_bloomer", "Late Bloomer", Common, 5, "[m]+1 Mult[/] at ball end per [k]10 seconds[/] the ball lasted", vec![r(BallEnd, Always, Mult(1.0, BallSeconds10))]),
        i("chalk_line", "Chalk Line", Common, 4, "Inlanes give [m]+2 Mult[/]", vec![r(Hit(&[Inlane]), Always, Mult(2.0, One))]),
        i("even_keel", "Even Keel", Common, 4, "[m]+6 Mult[/] at ball end if no multiball happened", vec![r(BallEnd, NoMultiball, Mult(6.0, One))]),
        i("first_strike", "First Strike", Common, 4, "The first hit of every ball gives [p]+80 Points[/]", vec![r(AnyHit, FirstHit, Pts(80.0, One))]),
        i("rubber_band", "Rubber Band", Common, 4, "Slingshot hits give [m]+0.5 Mult[/]", vec![r(Hit(&[Sling]), Always, Mult(0.5, One))]),
        i("pinwheel", "Pinwheel", Common, 4, "Every spin gives [p]+3 Points[/]", vec![r(Hit(&[Spin]), Always, Pts(3.0, One))]),
        i("bell_ringer", "Bell Ringer", Common, 5, "Completing the [k]Lane Set[/] gives [m]+5 Mult[/]", vec![r(Hit(&[Lanes]), Always, Mult(5.0, One))]),
        i("sharpshooter", "Sharpshooter", Common, 4, "Drop targets give [p]+25 Points[/]", vec![r(Hit(&[Drop]), Always, Pts(25.0, One))]),
        i("bargain_bin", "Bargain Bin", Common, 5, "Everything in the shop is [k]10% off[/]", vec![r(Passive, Always, Discount(0.9))]),
        i("night_shift", "Night Shift", Common, 5, "[m]+10 Mult[/] on the [k]last ball[/] of each table", vec![r(BallEnd, LastBall, Mult(10.0, One))]),
        i("warm_up", "Warm Up", Common, 4, "[m]+8 Mult[/] on the [k]first ball[/] of each table", vec![r(BallEnd, FirstBall, Mult(8.0, One))]),
        // ---------------------------------------------------------------- uncommon
        i("long_ball", "Long Ball", Uncommon, 6, "[k]+1 ball[/] per table", vec![r(Passive, Always, Balls(1))]),
        i("combo_breaker", "Combo Breaker", Uncommon, 7, "[x]×1.5 Mult[/] if you hit [k]5+ shot types[/] this ball", vec![r(BallEnd, KindsAtLeast(5), XMult(1.5, 0.0, One))]),
        i("momentum", "Momentum", Uncommon, 7, "Gains [m]+0.5 Mult[/] per ramp made. Applies at ball end", vec![r(Hit(&[Ramp]), Always, Grow(0.5)), r(BallEnd, Always, Mult(1.0, Stat))]),
        i("multiball_maniac", "Multiball Maniac", Uncommon, 7, "[x]×2 Mult[/] if a [k]multiball[/] happened this ball", vec![r(BallEnd, Multiball, XMult(2.0, 0.0, One))]),
        i("glass_cannon", "Glass Cannon", Uncommon, 6, "[x]×3 Mult[/] at ball end. [k]1 in 4[/] to shatter after each table", vec![r(BallEnd, Always, XMult(3.0, 0.0, One)), r(TableEnd, Chance(4), Destroy)]),
        i("hot_streak", "Hot Streak", Uncommon, 6, "Every [k]6-hit streak[/] gives [m]+2 Mult[/]", vec![r(Streak(6), Always, Mult(2.0, One))]),
        i("compound_interest", "Compound Interest", Uncommon, 6, "Raise the interest cap by [c]$5[/]", vec![r(Passive, Always, InterestCap(5))]),
        i("lucky_drain", "Lucky Drain", Uncommon, 6, "First drain of each table: [k]1 in 2[/] to save the ball", vec![r(Drain, FirstDrainOfTable, Save)]),
        i("fat_stack", "Fat Stack", Uncommon, 6, "[m]+1 Mult[/] per [c]$5[/] you hold, at ball end", vec![r(BallEnd, Always, Mult(1.0, Money5))]),
        i("wild_lanes", "Wild Lanes", Uncommon, 5, "Top lanes complete with any [k]2 of 3[/] lit", vec![r(Passive, Always, WildLanes)]),
        i("abacus", "Abacus", Uncommon, 6, "Every hit gives [p]+1 Points[/], plus 1 more per 40 hits made", vec![r(AnyHit, Always, Grow(1.0 / 40.0)), r(AnyHit, Always, Pts(1.0, Stat))]),
        i("bank_robber", "Bank Robber", Uncommon, 7, "Clearing the [k]Target Bank[/] gives [x]×1.5 Mult[/]", vec![r(Hit(&[Bank]), Always, XMult(1.5, 0.0, One))]),
        i("overdrive", "Overdrive", Uncommon, 6, "Bumpers kick [k]30% harder[/] and give [p]+15 Points[/]", vec![r(Passive, Always, BumperKick(1.3)), r(Hit(&[Bumper]), Always, Pts(15.0, One))]),
        i("swashbuckler", "Swashbuckler", Uncommon, 6, "Adds the [k]sell value[/] of your other items as [m]Mult[/]", vec![r(BallEnd, Always, Mult(1.0, OtherSellValue))]),
        i("metronome", "Metronome", Uncommon, 6, "[m]+1 Mult[/] per [k]10 spins[/] this ball, at ball end", vec![r(BallEnd, Always, Mult(1.0, SpinsThisBall10))]),
        i("featherweight", "Featherweight", Uncommon, 6, "The ball is [k]10% lighter[/]: slower to drain", vec![r(Passive, Always, Gravity(0.9))]),
        i("echo_chamber", "Echo Chamber", Uncommon, 6, "Combos give [m]+4 Mult[/]", vec![r(Hit(&[Combo]), Always, Mult(4.0, One))]),
        i("marathon", "Marathon", Uncommon, 7, "Gains [x]×0.2 Mult[/] for every ball that lasts 20s", vec![r(BallEnd, LongBall(20.0), Grow(0.2)), r(BallEnd, Always, XMult(1.0, 1.0, Stat))]),
        i("collector", "Collector", Uncommon, 6, "[m]+2 Mult[/] per item you own, at ball end", vec![r(BallEnd, Always, Mult(2.0, Items))]),
        i("dividend", "Dividend", Uncommon, 6, "Earn [c]$1[/] per 3 items you own after each table", vec![r(TableEnd, Always, Cash(1.0, Items3))]),
        i("pressure_plate", "Pressure Plate", Uncommon, 7, "Rollovers and inlanes give [x]×1.1 Mult[/]", vec![r(Hit(&[Rollover, Inlane]), Always, XMult(1.1, 0.0, One))]),
        i("ricochet", "Ricochet", Uncommon, 6, "Every [k]10th bumper[/] hit gives [x]×1.2 Mult[/]", vec![r(EveryNth(Bumper, 10), Always, XMult(1.2, 0.0, One))]),
        // ---------------------------------------------------------------- rare
        i("chrome_dome", "Chrome Dome", Rare, 8, "Gains [x]×0.004 Mult[/] per bumper hit, forever", vec![r(Hit(&[Bumper]), Always, Grow(0.004)), r(BallEnd, Always, XMult(1.0, 1.0, Stat))]),
        i("double_down", "Double Down", Rare, 9, "[x]×2 Mult[/] on the [k]last ball[/] of each table", vec![r(BallEnd, LastBall, XMult(2.0, 0.0, One))]),
        i("blueprint", "Blueprint", Rare, 10, "Copies the item [k]below it[/] in your rack", vec![r(Passive, Always, CopyBelow)]),
        i("brainstorm", "Brainstorm", Rare, 10, "Copies the [k]top item[/] in your rack", vec![r(Passive, Always, CopyTop)]),
        i("pinpoint", "Pinpoint", Rare, 8, "Combos give [x]×1.5 Mult[/]", vec![r(Hit(&[Combo]), Always, XMult(1.5, 0.0, One))]),
        i("black_hole", "Black Hole", Rare, 10, "[x]×1 Mult[/] for every 4 items you own", vec![r(BallEnd, Always, XMult(1.0, 0.25, Items))]),
        i("the_house", "The House", Rare, 9, "After each table, earn [c]$1[/] per [c]$4[/] held (max $15)", vec![r(TableEnd, Always, Cash(0.25, Money5))]),
        i("perpetual_motion", "Perpetual Motion", Rare, 9, "Every hit: [k]1 in 60[/] to award an [k]extra ball[/]", vec![r(AnyHit, Chance(60), ExtraBall)]),
        i("golden_ticket", "Golden Ticket", Rare, 8, "Target Bank earns [c]$5[/] and [m]+3 Mult[/]", vec![r(Hit(&[Bank]), Always, Cash(5.0, One)), r(Hit(&[Bank]), Always, Mult(3.0, One))]),
        i("infinity_ramp", "Infinity Ramp", Rare, 10, "Gains [x]×0.05 Mult[/] per ramp made, forever", vec![r(Hit(&[Ramp]), Always, Grow(0.05)), r(BallEnd, Always, XMult(1.0, 1.0, Stat))]),
    ]
}

// ------------------------------------------------------------------ cards
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CardEff {
    Level(&'static [Shot], u32),
    Enhance(Enh),
    Multiball,
    SafetyNet,
    Photocopy,
    Hermit,
    PrismWheel,
    Recycler,
    Overclock,
    FortuneCookie,
    Sledgehammer,
    TuneUp,
}

pub struct CardDef { pub id: &'static str, pub name: &'static str, pub cost: i64, pub desc: &'static str, pub eff: CardEff, pub play_only: bool, pub chip: bool }

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Enh { Gold, Glass, Bonus, Mult, Lucky }
pub fn enh_name(e: Enh) -> &'static str { match e { Enh::Gold => "Gold", Enh::Glass => "Glass", Enh::Bonus => "Bonus", Enh::Mult => "Mult", Enh::Lucky => "Lucky" } }
pub fn enh_desc(e: Enh) -> &'static str {
    match e {
        Enh::Gold => "Earns [c]$1[/] every 4 hits",
        Enh::Glass => "[x]×1.25 Mult[/] per hit, 1 in 10 to shatter",
        Enh::Bonus => "[p]+40 Points[/] per hit",
        Enh::Mult => "[m]+1 Mult[/] per hit",
        Enh::Lucky => "1 in 5: [m]+5 Mult[/]. 1 in 15: [c]$5[/]",
    }
}

pub fn cards() -> Vec<CardDef> {
    let c = |id, name, cost, desc, eff, play_only, chip| CardDef { id, name, cost, desc, eff, play_only, chip };
    use CardEff::*;
    vec![
        c("chip_bumper", "Bumper Chip", 3, "Level up [k]Bumpers[/]", Level(&[Bumper], 1), false, true),
        c("chip_lower", "Lower Chip", 3, "Level up [k]Slingshots & Inlanes[/]", Level(&[Sling, Inlane], 1), false, true),
        c("chip_spin", "Spinner Chip", 3, "Level up the [k]Spinner[/]", Level(&[Spin], 1), false, true),
        c("chip_lanes", "Lane Chip", 3, "Level up [k]Rollovers & Lane Set[/]", Level(&[Rollover, Lanes], 1), false, true),
        c("chip_bank", "Bank Chip", 3, "Level up [k]Drop Targets & Bank[/]", Level(&[Drop, Bank], 1), false, true),
        c("chip_standup", "Standup Chip", 3, "Level up [k]Standups[/]", Level(&[Standup], 1), false, true),
        c("chip_ramp", "Ramp Chip", 3, "Level up the [k]Ramp[/]", Level(&[Ramp], 1), false, true),
        c("chip_orbit", "Orbit Chip", 3, "Level up the [k]Orbit[/]", Level(&[Orbit], 1), false, true),
        c("chip_combo", "Combo Chip", 3, "Level up [k]Combos[/]", Level(&[Combo], 1), false, true),
        c("gold_leaf", "Gold Leaf", 3, "Turn a random table piece [k]Gold[/]", Enhance(Enh::Gold), false, false),
        c("glass_coat", "Glass Coat", 3, "Turn a random table piece [k]Glass[/]", Enhance(Enh::Glass), false, false),
        c("blue_paint", "Blue Paint", 3, "Turn a random table piece [k]Bonus[/]", Enhance(Enh::Bonus), false, false),
        c("red_paint", "Red Paint", 3, "Turn a random table piece [k]Mult[/]", Enhance(Enh::Mult), false, false),
        c("lucky_charm", "Lucky Charm", 3, "Turn a random table piece [k]Lucky[/]", Enhance(Enh::Lucky), false, false),
        c("multiball", "Multiball", 4, "Use during play: launch an [k]extra ball[/]", Multiball, true, false),
        c("safety_net", "Safety Net", 3, "Use during play: [k]Ball Save[/] for 15 seconds", SafetyNet, true, false),
        c("photocopy", "Photocopy", 5, "Copy a random item you own", Photocopy, false, false),
        c("hermit", "The Hermit", 4, "Double your money (max [c]+$20[/])", Hermit, false, false),
        c("prism_wheel", "Prism Wheel", 4, "[k]1 in 3[/]: a random item gains an edition", PrismWheel, false, false),
        c("recycler", "Recycler", 3, "Create [k]2 random Chips[/]", Recycler, false, false),
        c("overclock", "Overclock", 5, "Level up a random shot [k]3 times[/]", Overclock, false, false),
        c("fortune_cookie", "Fortune Cookie", 2, "Gain [c]$1 to $10[/]", FortuneCookie, false, false),
        c("sledgehammer", "Sledgehammer", 2, "Destroy a random item, gain [c]3× its sell value[/]", Sledgehammer, false, false),
        c("tune_up", "Tune-Up", 4, "Flippers swing [k]5% harder[/] for the rest of the run", TuneUp, false, false),
    ]
}

// ------------------------------------------------------------------ vouchers
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum VEff { ShopItems(u32), Balls(i32), RerollBase(i64), InterestCap(i64), Flipper(f64), BallSave(f64), Discount(f64), ShopCards(u32), TiltTolerance(f64), Luck(u32), StartCash(i64), PriceFreeze }
pub struct VoucherDef { pub id: &'static str, pub name: &'static str, pub cost: i64, pub desc: &'static str, pub eff: VEff }
pub fn vouchers() -> Vec<VoucherDef> {
    use VEff::*;
    let v = |id, name, cost, desc, eff| VoucherDef { id, name, cost, desc, eff };
    vec![
        v("overstock", "Overstock", 10, "[k]+1 item[/] on offer in the shop", ShopItems(1)),
        v("overstock2", "Overstock Plus", 14, "[k]+1 more item[/] on offer in the shop", ShopItems(1)),
        v("extra_ball", "Extra Ball", 10, "[k]+1 ball[/] per table", Balls(1)),
        v("surplus", "Reroll Surplus", 10, "Rerolls cost [c]$2[/] less", RerollBase(-2)),
        v("seed_money", "Seed Money", 10, "Raise the interest cap by [c]$5[/]", InterestCap(5)),
        v("hydraulics", "Hydraulics", 10, "Flippers swing [k]15% harder[/]", Flipper(1.15)),
        v("airbag", "Airbag", 10, "[k]Ball Save[/] for the first 5 seconds of every ball", BallSave(5.0)),
        v("clearance", "Clearance Sale", 10, "Everything in the shop is [k]25% off[/]", Discount(0.75)),
        v("magnifier", "Magnifier", 10, "[k]+1 card[/] on offer in the shop", ShopCards(1)),
        v("tilt_guard", "Tilt Guard", 8, "Nudge [k]50% more[/] before tilting", TiltTolerance(1.5)),
        v("horseshoe", "Horseshoe", 12, "Every [k]chance[/] roll is twice as likely", Luck(2)),
        v("price_freeze", "Price Freeze", 12, "Item prices no longer rise as your rack grows", PriceFreeze),
    ]
}

// ------------------------------------------------------------------ bosses & tags & cabinets
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Boss { Lead, Mute, Limp, Mirror, Drift, Short, Barricade, Fog, Wall, Slick, Tax, Sticky, Jitter, Miser, Crown }
pub struct BossDef { pub id: Boss, pub name: &'static str, pub desc: &'static str, pub color: u32 }
pub fn bosses() -> Vec<BossDef> {
    use Boss::*;
    let b = |id, name, desc, color| BossDef { id, name, desc, color };
    vec![
        b(Lead, "The Lead", "The ball is 30% heavier", 0x9a99b3),
        b(Mute, "The Mute", "Bumpers score nothing", 0x6f6d8c),
        b(Limp, "The Limp", "The left flipper is weak", 0xa596ec),
        b(Mirror, "The Mirror", "Flipper buttons are swapped", 0x49c6ec),
        b(Drift, "The Drift", "The table leans to the left", 0xe89a1c),
        b(Short, "The Short", "One fewer ball", 0xee4040),
        b(Barricade, "The Barricade", "The ramp is closed", 0xffcc4d),
        b(Fog, "The Fog", "The lights are out", 0x4a4766),
        b(Wall, "The Wall", "Extra large target", 0x7d68d4),
        b(Slick, "The Slick", "Bumpers and slingshots kick twice as hard", 0x3fc46a),
        b(Tax, "The Tax", "Lose $1 every time a ball drains", 0xffcc4d),
        b(Sticky, "The Sticky", "Flippers return slowly", 0xa8693a),
        b(Jitter, "The Jitter", "The table shakes every few seconds", 0xff6aa8),
        b(Miser, "The Miser", "Items earn no money this table", 0xb8650f),
        b(Crown, "Neon Crown", "Heavier ball and a huge target", 0xff3b8c),
    ]
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tag { Cash, Chips, Rare, Polish, Level, Economy }
pub fn tag_info(t: Tag) -> (&'static str, &'static str) {
    match t {
        Tag::Cash => ("Cash Tag", "Gain [c]$8[/]"),
        Tag::Chips => ("Chip Tag", "Gain [k]2 random Chips[/]"),
        Tag::Rare => ("Rare Tag", "Gain a random [k]rare[/] item"),
        Tag::Polish => ("Polish Tag", "A random item becomes [k]Foil[/]"),
        Tag::Level => ("Level Tag", "Level up your most hit shot [k]twice[/]"),
        Tag::Economy => ("Economy Tag", "Double your money (max [c]+$30[/])"),
    }
}
pub const TAGS: [Tag; 6] = [Tag::Cash, Tag::Chips, Tag::Rare, Tag::Polish, Tag::Level, Tag::Economy];

pub struct CabinetDef { pub id: &'static str, pub name: &'static str, pub desc: &'static str, pub unlock: &'static str }
pub fn cabinets() -> Vec<CabinetDef> {
    let c = |id, name, desc, unlock| CabinetDef { id, name, desc, unlock };
    vec![
        c("classic", "Classic", "The standard machine.", ""),
        c("red", "Red Cabinet", "[k]+1 ball[/] per table.", "Reach ante 3"),
        c("blue", "Blue Cabinet", "Start with a [k]Safety Net[/] and a [k]Multiball[/].", "Beat a boss table"),
        c("gold", "Gold Cabinet", "Start with [c]$15[/]. Interest cap [c]$10[/].", "Reach ante 5"),
        c("glass", "Glass Cabinet", "All bumpers start [k]Glass[/].", "Reach ante 4"),
        c("black", "Black Cabinet", "Items cost [c]$1[/] less, [k]-1 ball[/] per table.", "Win a run"),
    ]
}

/// base score target per ante (small table); big ×1.5, boss ×2. Endless after ante 8.
pub fn ante_target(ante: u32) -> f64 {
    const BASE: [f64; 8] = [1000.0, 2800.0, 7000.0, 15000.0, 32000.0, 60000.0, 105000.0, 180000.0];
    if ante <= 8 { BASE[(ante.max(1) - 1) as usize] } else { BASE[7] * 1.8f64.powi(ante as i32 - 8) }
}
