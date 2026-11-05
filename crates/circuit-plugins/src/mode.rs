//! Build modes (`modes/*.toml`).

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BuildMode {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub description: String,
    /// `plugin.id/*` patterns of blocks that appear in this mode.
    pub blocks: Vec<String>,
    pub runtime: String,
    pub runtime_libs: Vec<String>,
    pub exporter: Option<String>,
    pub template: Option<String>,
    /// The environment variable an exported bot reads its token from
    /// (`FLUXER_BOT_TOKEN`). Unset: `DISCORD_TOKEN`.
    pub token_env: Option<String>,
    /// What the mode's projects have: `slashCommands` shows the Slash Command
    /// Builder.
    pub features: Vec<String>,
    pub categories: Vec<Category>,
    /// Settings page fields, keyed by setting id.
    pub settings: IndexMap<String, Value>,
    /// npm packages the runtime libraries need.
    pub dependencies: IndexMap<String, String>,
    /// Intents (as blocks name them) that have to be turned on by hand for
    /// the bot, with what the platform calls them: Bot settings lists the
    /// ones the project's blocks need.
    pub privileged_intents: IndexMap<String, String>,
    /// Where they are turned on; `{appId}` is the bot's application id.
    pub intents_page: Option<String>,
    #[serde(skip)]
    pub plugin: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
}

impl BuildMode {
    pub fn token_env(&self) -> &str {
        self.token_env.as_deref().unwrap_or("DISCORD_TOKEN")
    }

    pub fn has(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }

    /// Whether a block from `plugin` with `block_id` belongs to this mode.
    pub fn includes(&self, plugin: &str, block_id: &str) -> bool {
        self.blocks.iter().any(|pattern| match pattern.split_once('/') {
            Some((p, "*")) => p == plugin,
            Some((p, id)) => p == plugin && id == block_id,
            None => pattern == plugin,
        })
    }

    pub fn category(&self, id: &str) -> Option<&Category> {
        self.categories.iter().find(|c| c.id.eq_ignore_ascii_case(id) || c.name.eq_ignore_ascii_case(id))
    }

    /// Position of a category in the palette; unknown categories sort last.
    pub fn category_rank(&self, id: &str) -> usize {
        self.categories
            .iter()
            .position(|c| c.id.eq_ignore_ascii_case(id) || c.name.eq_ignore_ascii_case(id))
            .unwrap_or(usize::MAX)
    }
}
