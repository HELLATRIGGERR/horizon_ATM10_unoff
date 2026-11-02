//! `config/fluxnetworks-server.toml`, the settings the port reads. A missing
//! file or key, or a value out of the mod's range, keeps the default, as
//! NeoForge's config spec corrects it. ATM10's file holds the mod's defaults.
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct FluxConfig {
    /// `energy.defaultLimit`: a new plug's or point's transfer limit, FE/t.
    pub default_limit: i64,
    /// `networks.maximumPerPlayer`: networks a player may own; -1 is no limit.
    pub maximum_per_player: i32,
    /// `blacklist.blockBlacklistStrings`: blocks devices don't connect to.
    pub block_blacklist: Vec<String>,
    /// Whether the settings came from the file.
    pub from_file: bool,
}

impl Default for FluxConfig {
    fn default() -> Self {
        Self {
            default_limit: 800_000,
            maximum_per_player: 5,
            block_blacklist: vec!["actuallyadditions:block_phantom_energyface".to_owned()],
            from_file: false,
        }
    }
}

impl FluxConfig {
    /// Reads `<config>/fluxnetworks-server.toml`.
    pub fn load(config: &Path) -> Self {
        std::fs::read_to_string(config.join("fluxnetworks-server.toml"))
            .map(|text| Self { from_file: true, ..Self::parse(&text) })
            .unwrap_or_default()
    }

    /// The settings in a file's text.
    pub fn parse(text: &str) -> Self {
        let mut out = Self::default();
        let Ok(table) = text.parse::<toml::Table>() else {
            return out;
        };
        let value = |section: &str, key: &str| table.get(section)?.get(key).cloned();
        if let Some(limit) = value("energy", "defaultLimit").and_then(|v| v.as_integer())
            && limit >= 0
        {
            out.default_limit = limit;
        }
        if let Some(max) = value("networks", "maximumPerPlayer").and_then(|v| v.as_integer())
            && (-1..=i64::from(i32::MAX)).contains(&max)
        {
            out.maximum_per_player = max as i32;
        }
        if let Some(list) = value("blacklist", "blockBlacklistStrings")
            && let Some(list) = list.as_array()
        {
            out.block_blacklist = list
                .iter()
                .filter_map(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
        }
        out
    }

    /// Whether devices refuse the block `id` as a neighbour.
    pub fn blacklisted(&self, id: &str) -> bool {
        self.block_blacklist.iter().any(|b| b == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_atm10_writes() {
        let c = FluxConfig::parse(
            "[networks]\n\tmaximumPerPlayer = 7\n[blacklist]\n\tblockBlacklistStrings = [\"a:b\"]\n\titemBlackListStrings = [\"\"]\n[energy]\n\tdefaultLimit = 1000\n",
        );
        assert_eq!(c.default_limit, 1000);
        assert_eq!(c.maximum_per_player, 7);
        assert!(c.blacklisted("a:b"));
        assert!(!c.blacklisted("minecraft:chest"));
    }

    #[test]
    fn out_of_range_keeps_the_default() {
        let c = FluxConfig::parse("[networks]\nmaximumPerPlayer = -2\n[energy]\ndefaultLimit = -1\n");
        assert_eq!(c, FluxConfig::default());
        assert_eq!(FluxConfig::parse("not toml ["), FluxConfig::default());
        assert_eq!(FluxConfig::default().default_limit, 800_000);
    }

    #[test]
    fn the_default_blacklist_is_the_mods() {
        assert!(FluxConfig::default().blacklisted("actuallyadditions:block_phantom_energyface"));
        // A file's list replaces it, also by an empty one.
        assert!(!FluxConfig::parse("[blacklist]\nblockBlacklistStrings = []\n").blacklisted("actuallyadditions:block_phantom_energyface"));
    }

    #[test]
    fn only_a_file_read_says_so() {
        let dir = std::env::temp_dir().join(format!("flux-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!FluxConfig::load(&dir).from_file, "no file");
        std::fs::write(dir.join("fluxnetworks-server.toml"), "[energy]\ndefaultLimit = 7\n").unwrap();
        let read = FluxConfig::load(&dir);
        assert!(read.from_file);
        assert_eq!(read.default_limit, 7);
        std::fs::remove_file(dir.join("fluxnetworks-server.toml")).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}
