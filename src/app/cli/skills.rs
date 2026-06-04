use {
    clap::{Args, Subcommand},
    serde::Serialize,
    std::{error::Error, fmt, io},
};

const SKILL_NAME_PREFIX: &str = "seagrass-";
const RAW_SKILL_BASE_URL: &str = "https://raw.githubusercontent.com/heyAyushh/seagrass/main";
const SKILLS_CATALOG_PATH: &str = "skills/README.md";
const BINARY_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(super) const SKILLS_HELP: &str = "\
For agents:
  Current instructions: seagrass skills get lint
  All topics: seagrass skills list --json
  Full reference: seagrass skills get lint --full
  Skill paths: seagrass skills path --json

Output:
  list prints available workflow topics, get prints version-matched Markdown,
  path locates bundled skill references. Add --json for machine-readable output.";

#[derive(Debug, Args)]
pub(super) struct SkillsCommand {
    #[command(subcommand)]
    command: SkillsSubcommand,
}

impl SkillsCommand {
    pub(super) fn run(self) -> Result<bool, Box<dyn Error>> {
        match self.command {
            SkillsSubcommand::List(command) => command.run(),
            SkillsSubcommand::Get(command) => command.run(),
            SkillsSubcommand::Path(command) => command.run(),
        }?;
        Ok(false)
    }
}

#[derive(Debug, Subcommand)]
enum SkillsSubcommand {
    /// List available agent workflow topics.
    List(ListCommand),
    /// Print one workflow topic's current instructions.
    Get(GetCommand),
    /// Print catalog or topic paths and raw source URLs.
    Path(PathCommand),
}

#[derive(Debug, Args)]
struct ListCommand {
    /// Print machine-readable JSON.
    #[arg(long = "json")]
    json: bool,
}

impl ListCommand {
    fn run(self) -> Result<(), Box<dyn Error>> {
        let catalog = SkillCatalog::new();
        if self.json {
            write_json(&catalog)?;
        } else {
            println!("{}", catalog.to_markdown());
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct GetCommand {
    /// Topic name, for example lint or seagrass-lint.
    #[arg(value_name = "NAME")]
    name: String,

    /// Print the full bundled SKILL.md instead of the compact workflow summary.
    #[arg(long = "full")]
    full: bool,

    /// Print machine-readable JSON.
    #[arg(long = "json")]
    json: bool,
}

impl GetCommand {
    fn run(self) -> Result<(), Box<dyn Error>> {
        let topic = topic_by_name(&self.name)?;
        let output = SkillOutput::new(topic, self.full);
        if self.json {
            write_json(&output)?;
        } else {
            print!("{}", output.markdown);
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
struct PathCommand {
    /// Optional topic name, for example lint or seagrass-lint.
    #[arg(value_name = "NAME")]
    name: Option<String>,

    /// Print machine-readable JSON.
    #[arg(long = "json")]
    json: bool,
}

impl PathCommand {
    fn run(self) -> Result<(), Box<dyn Error>> {
        let output = match self.name {
            Some(name) => SkillPathOutput::for_topic(topic_by_name(&name)?),
            None => SkillPathOutput::for_catalog(),
        };
        if self.json {
            write_json(&output)?;
        } else {
            println!("{}", output.to_markdown());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillCatalog {
    schema_version: u8,
    binary_version: &'static str,
    catalog_path: &'static str,
    catalog_raw_url: String,
    topics: Vec<SkillSummary>,
}

impl SkillCatalog {
    fn new() -> Self {
        Self {
            schema_version: 1,
            binary_version: BINARY_VERSION,
            catalog_path: SKILLS_CATALOG_PATH,
            catalog_raw_url: raw_url(SKILLS_CATALOG_PATH),
            topics: SKILL_TOPICS.iter().map(SkillSummary::from_topic).collect(),
        }
    }

    fn to_markdown(&self) -> String {
        let mut markdown = String::from("# Seagrass Agent Skills\n\n");
        markdown.push_str(&format!(
            "Installed Seagrass version: `{}`\n\n",
            self.binary_version
        ));
        markdown.push_str("Current installed topics:\n\n");
        for topic in &self.topics {
            markdown.push_str(&format!(
                "- `{}` (`{}`): {}\n",
                topic.name, topic.skill, topic.purpose
            ));
        }
        markdown.push_str("\nCommands:\n\n");
        markdown.push_str("```sh\n");
        markdown.push_str("seagrass skills list --json\n");
        markdown.push_str("seagrass skills get lint\n");
        markdown.push_str("seagrass skills get lint --full\n");
        markdown.push_str("seagrass skills path lint --json\n");
        markdown.push_str("```\n");
        markdown
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillSummary {
    name: &'static str,
    skill: &'static str,
    description: &'static str,
    purpose: &'static str,
    path: &'static str,
    raw_url: String,
    get_command: String,
    full_command: String,
}

impl SkillSummary {
    fn from_topic(topic: &'static SkillTopic) -> Self {
        Self {
            name: topic.name,
            skill: topic.skill,
            description: topic.description,
            purpose: topic.purpose,
            path: topic.path,
            raw_url: raw_url(topic.path),
            get_command: format!("seagrass skills get {}", topic.name),
            full_command: format!("seagrass skills get {} --full", topic.name),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillOutput {
    schema_version: u8,
    binary_version: &'static str,
    full: bool,
    summary: SkillSummary,
    markdown: String,
}

impl SkillOutput {
    fn new(topic: &'static SkillTopic, full: bool) -> Self {
        let markdown = if full {
            topic.markdown.to_string()
        } else {
            topic.compact_markdown()
        };
        Self {
            schema_version: 1,
            binary_version: BINARY_VERSION,
            full,
            summary: SkillSummary::from_topic(topic),
            markdown,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillPathOutput {
    schema_version: u8,
    binary_version: &'static str,
    catalog_path: &'static str,
    catalog_raw_url: String,
    topics: Vec<SkillPathSummary>,
}

impl SkillPathOutput {
    fn for_catalog() -> Self {
        Self {
            schema_version: 1,
            binary_version: BINARY_VERSION,
            catalog_path: SKILLS_CATALOG_PATH,
            catalog_raw_url: raw_url(SKILLS_CATALOG_PATH),
            topics: SKILL_TOPICS
                .iter()
                .map(|topic| SkillPathSummary {
                    name: topic.name,
                    skill: topic.skill,
                    path: topic.path,
                    raw_url: raw_url(topic.path),
                })
                .collect(),
        }
    }

    fn for_topic(topic: &'static SkillTopic) -> Self {
        Self {
            schema_version: 1,
            binary_version: BINARY_VERSION,
            catalog_path: SKILLS_CATALOG_PATH,
            catalog_raw_url: raw_url(SKILLS_CATALOG_PATH),
            topics: vec![SkillPathSummary {
                name: topic.name,
                skill: topic.skill,
                path: topic.path,
                raw_url: raw_url(topic.path),
            }],
        }
    }

    fn to_markdown(&self) -> String {
        let mut markdown = format!(
            "version {}\n{}\n{}\n",
            self.binary_version, self.catalog_path, self.catalog_raw_url
        );
        for topic in &self.topics {
            markdown.push_str(&format!("{}\n{}\n", topic.path, topic.raw_url));
        }
        markdown
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillPathSummary {
    name: &'static str,
    skill: &'static str,
    path: &'static str,
    raw_url: String,
}

#[derive(Debug)]
struct SkillTopic {
    name: &'static str,
    skill: &'static str,
    description: &'static str,
    purpose: &'static str,
    path: &'static str,
    markdown: &'static str,
}

impl SkillTopic {
    fn compact_markdown(&self) -> String {
        format!(
            "# {skill}\n\n{description}\n\nInstalled Seagrass version: `{version}`\n\nPurpose: {purpose}\n\nCommands:\n\n```sh\nseagrass skills get {name} --full\nseagrass skills path {name} --json\n```\n",
            skill = self.skill,
            description = self.description,
            version = BINARY_VERSION,
            purpose = self.purpose,
            name = self.name,
        )
    }
}

const SKILL_TOPICS: &[SkillTopic] = &[
    SkillTopic {
        name: "install",
        skill: "seagrass-install",
        description: "Install and configure Seagrass in editors or CI.",
        purpose: "Install binary, configure the editor, and run the smoke check.",
        path: "skills/seagrass-install/SKILL.md",
        markdown: include_str!("../../../skills/seagrass-install/SKILL.md"),
    },
    SkillTopic {
        name: "lint",
        skill: "seagrass-lint",
        description: "Run diagnostics and triage findings by severity and topic.",
        purpose: "Surface Seagrass issues from a file, directory, stdin, or SARIF run.",
        path: "skills/seagrass-lint/SKILL.md",
        markdown: include_str!("../../../skills/seagrass-lint/SKILL.md"),
    },
    SkillTopic {
        name: "explain",
        skill: "seagrass-explain",
        description: "Explain a diagnostic topic from the authoritative lint docs.",
        purpose: "Resolve a topic, code, or message to docs, false-positive matrix, and suppression forms.",
        path: "skills/seagrass-explain/SKILL.md",
        markdown: include_str!("../../../skills/seagrass-explain/SKILL.md"),
    },
    SkillTopic {
        name: "suppress",
        skill: "seagrass-suppress",
        description: "Suppress a finding at the narrowest correct scope.",
        purpose: "Pick the right line, item, file, or workspace suppression form.",
        path: "skills/seagrass-suppress/SKILL.md",
        markdown: include_str!("../../../skills/seagrass-suppress/SKILL.md"),
    },
    SkillTopic {
        name: "debug-fp",
        skill: "seagrass-debug-fp",
        description: "Investigate and minimize a Seagrass false positive.",
        purpose: "Confirm a false-positive contract breach and produce a regression fixture.",
        path: "skills/seagrass-debug-fp/SKILL.md",
        markdown: include_str!("../../../skills/seagrass-debug-fp/SKILL.md"),
    },
    SkillTopic {
        name: "audit",
        skill: "seagrass-audit",
        description: "Run a full Seagrass quality audit on a Solana project.",
        purpose: "Produce a ranked production-readiness punch list from diagnostics and suppression review.",
        path: "skills/seagrass-audit/SKILL.md",
        markdown: include_str!("../../../skills/seagrass-audit/SKILL.md"),
    },
];

fn topic_by_name(name: &str) -> Result<&'static SkillTopic, SkillLookupError> {
    let normalized = normalized_topic_name(name);
    SKILL_TOPICS
        .iter()
        .find(|topic| topic.name == normalized || topic.skill == name)
        .ok_or_else(|| SkillLookupError::new(name.to_string()))
}

fn normalized_topic_name(name: &str) -> &str {
    name.strip_prefix(SKILL_NAME_PREFIX).unwrap_or(name)
}

fn raw_url(path: &str) -> String {
    format!("{RAW_SKILL_BASE_URL}/{path}")
}

fn write_json(value: &impl Serialize) -> Result<(), Box<dyn Error>> {
    serde_json::to_writer_pretty(io::stdout(), value)?;
    println!();
    Ok(())
}

#[derive(Debug)]
struct SkillLookupError {
    name: String,
}

impl SkillLookupError {
    fn new(name: String) -> Self {
        Self { name }
    }
}

impl fmt::Display for SkillLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown Seagrass skill `{}`. Run `seagrass skills list --json` to inspect available topics.",
            self.name
        )
    }
}

impl Error for SkillLookupError {}

#[cfg(test)]
mod tests {
    use {super::*, clap::CommandFactory};

    #[test]
    fn skill_list_exposes_all_agent_topics() {
        let catalog = SkillCatalog::new();

        assert_eq!(catalog.binary_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(catalog.topics.len(), SKILL_TOPICS.len());
        assert!(catalog.topics.iter().any(|topic| topic.name == "lint"));
        assert!(catalog
            .topics
            .iter()
            .all(|topic| topic.get_command.starts_with("seagrass skills get ")));
    }

    #[test]
    fn skill_lookup_accepts_short_and_full_names() {
        assert_eq!(topic_by_name("lint").unwrap().skill, "seagrass-lint");
        assert_eq!(topic_by_name("seagrass-debug-fp").unwrap().name, "debug-fp");
    }

    #[test]
    fn skill_get_uses_progressive_disclosure() {
        let topic = topic_by_name("lint").unwrap();
        let compact = SkillOutput::new(topic, false);
        let full = SkillOutput::new(topic, true);

        assert_eq!(compact.binary_version, env!("CARGO_PKG_VERSION"));
        assert!(compact.markdown.contains("seagrass skills get lint --full"));
        assert!(compact.markdown.contains("Installed Seagrass version"));
        assert!(!compact.markdown.contains("## When to use"));
        assert!(full.markdown.contains("## When to use"));
        assert!(full.markdown.contains("seagrass diagnostics <path> --json"));
    }

    #[test]
    fn skill_path_outputs_catalog_and_raw_urls() {
        let output = SkillPathOutput::for_topic(topic_by_name("audit").unwrap());

        assert_eq!(output.binary_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(output.catalog_path, SKILLS_CATALOG_PATH);
        assert_eq!(output.topics[0].path, "skills/seagrass-audit/SKILL.md");
        assert!(output.topics[0]
            .raw_url
            .contains("raw.githubusercontent.com"));
    }

    #[test]
    fn skills_help_matches_agent_browser_pattern() {
        let mut command = super::super::Cli::command();
        let skills_command = command
            .find_subcommand_mut("skills")
            .expect("skills subcommand");
        let mut help = Vec::new();

        skills_command.write_long_help(&mut help).unwrap();
        let help = String::from_utf8(help).unwrap();

        assert!(help.contains("seagrass skills list --json"));
        assert!(help.contains("seagrass skills get lint --full"));
        assert!(help.contains("seagrass skills path --json"));
    }

    #[test]
    fn skills_catalog_markdown_is_embedded() {
        let catalog_markdown = include_str!("../../../skills/README.md");

        assert!(catalog_markdown.contains("seagrass-install"));
    }
}
