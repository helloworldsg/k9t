use crate::config::CustomCommand;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Quit,
    Custom(CustomCommand),
    Unknown(String),
}

/// An item displayed in the command palette, built from built-ins + custom commands.
#[derive(Debug, Clone)]
pub struct CommandItem {
    /// The `:name` the user would type (e.g. "ns", "pf").
    pub name: String,
    /// Short description for the right column.
    pub description: String,
    /// Whether this is a user-defined custom command.
    pub is_custom: bool,
    /// The resolved command to execute when selected.
    pub command: Command,
}

impl CommandItem {
    /// Build the full list of palette items from built-in commands + user custom commands.
    pub fn build_list(custom_commands: &[CustomCommand]) -> Vec<CommandItem> {
        let mut items: Vec<CommandItem> = Vec::new();

        // Built-in commands
        items.push(CommandItem {
            name: "q".to_string(),
            description: "Quit k9t".to_string(),
            is_custom: false,
            command: Command::Quit,
        });

        // Custom commands from config
        for cc in custom_commands {
            let desc = cc.description.clone().unwrap_or_else(|| cc.command.clone());
            items.push(CommandItem {
                name: cc.name.clone(),
                description: desc,
                is_custom: true,
                command: Command::Custom(cc.clone()),
            });
        }

        items
    }

    /// Simple fuzzy match: query characters must appear in order within the
    /// `name` or `description` (case-insensitive). Empty query matches everything.
    pub fn fuzzy_matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let q = query.to_lowercase();
        let hay = format!("{} {}", self.name, self.description).to_lowercase();
        fuzzy_match(&q, &hay)
    }
}

/// Fuzzy match: every character in `needle` must appear in `haystack` in order.
fn fuzzy_match(needle: &str, haystack: &str) -> bool {
    let mut hay_chars = haystack.chars();
    for nc in needle.chars() {
        loop {
            match hay_chars.next() {
                Some(hc) if hc == nc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

impl Command {
    /// Parse a command-mode input string against built-in commands and custom commands.
    ///
    /// Custom commands are tried first (by name), then built-in aliases.
    pub fn parse(input: &str, custom_commands: &[CustomCommand]) -> Self {
        let input = input.trim();

        // Check custom commands by name first
        if let Some(cmd) = custom_commands.iter().find(|c| c.name == input) {
            return Command::Custom(cmd.clone());
        }

        // Built-in commands
        match input {
            "q" | "quit" | "exit" => Command::Quit,
            _ => Command::Unknown(input.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_match_basic() {
        assert!(fuzzy_match("pf", "port-forward"));
        // Case is handled by the caller (fuzzy_matches lowercases both)
        assert!(fuzzy_match("ns", "switch namespace"));
        assert!(!fuzzy_match("xyz", "switch namespace"));
        assert!(fuzzy_match("", "anything"));
    }

    #[test]
    fn command_item_fuzzy_matches() {
        let item = CommandItem {
            name: "pf".to_string(),
            description: "Port-forward pod".to_string(),
            is_custom: true,
            command: Command::Unknown("x".to_string()),
        };
        assert!(item.fuzzy_matches(""));
        assert!(item.fuzzy_matches("pf"));
        assert!(item.fuzzy_matches("port"));
        assert!(!item.fuzzy_matches("xyz"));
    }
}
