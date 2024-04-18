use serde::{Deserialize, Serialize};

// https://galaxy-integrations-python-api.readthedocs.io/en/latest/platforms.html
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub enum GalaxyPlatform {
    // Testing platform
    Test,
    // Generic gamesdb id
    Generic,
    GOG,
    Uplay,
    Steam,
    Origin,
    XboxOne,
    Psn,
    BattleNet,
    Epic,
    Bethesda,
    Paradox,
    Humble,
    Kartridge,
    Itch,
    NSwitch,
    NWiiU,
    NWii,
    NCube,
    Riot,
    Wargaming,
    NGameBoy,
    Atari,
    Amiga,
    Snes,
    Beamdog,
    D2D,
    Discord,
    DotEmu,
    Gamehouse,
    GMG,
    WePlay,
    ZX,
    Vision,
    NES,
    SMS,
    C64,
    PCE,
    SegaG,
    Neo,
    Sega32,
    SegaCD,
    #[serde(rename = "3do")]
    I3DO,
    Saturn,
    PsX,
    Ps2,
    N64,
    Jaguar,
    DC,
    XboxOG,
    Amazon,
    GG,
    Egg,
    BB,
    GameUK,
    Fanatical,
    PlayAsia,
    Stadia,
    Arc,
    Eso,
    Glyph,
    AionL,
    Blade,
    Gw,
    Gw2,
    Lin2,
    FFXI,
    FFXIV,
    TotalWar,
    WinStore,
    EliTes,
    Star,
    Psp,
    PsVita,
    NDS,
    #[serde(rename = "3ds")]
    N3DS,
    PathOfExile,
    Twitch,
    Minecraft,
    GameSessions,
    Nuuvem,
    FxStore,
    IndieGala,
    Playfire,
    Oculus,
    Rockstar,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GalaxyLibraryItem {
    pub platform_id: GalaxyPlatform,
    pub external_id: String,
    pub origin: String, // This seems to always be "client"
    pub date_created: u32,
    pub owned_since: Option<u32>,
    pub certificate: String,
    pub hidden: bool,
    pub owned: bool,
}

#[derive(Deserialize, Debug)]
pub struct GalaxyLibraryResponse {
    pub total_count: u32,
    pub limit: u32,
    pub items: Vec<GalaxyLibraryItem>,
    pub next_page_token: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct OwnedProductsResponse {
    pub owned: Vec<u64>,
}
