use anyhow::{bail, Result};

use super::cli::BrowserArgs;
use super::repl::vendor::{detect_cursor, spawn_cursor, CursorRelay};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiMode {
    Off,
    Cursor,
}

pub fn resolve_ai_mode(args: &BrowserArgs) -> Result<AiMode> {
    if args.no_ai {
        return Ok(AiMode::Off);
    }

    match args.provider.as_str() {
        "auto" | "cursor" => {
            if detect_cursor() {
                Ok(AiMode::Cursor)
            } else if args.provider == "auto" {
                bail!(
                    "Cursor agent CLI not found.\n\
                     Install Cursor CLI (`agent`), run `agent login`, or retry with --no-ai."
                )
            } else {
                bail!(
                    "Cursor agent CLI not found (provider=cursor).\n\
                     Set PAGEMD_CURSOR_AGENT or install `agent`."
                )
            }
        }
        other => bail!("unknown --provider {other:?} (expected auto or cursor)"),
    }
}

pub fn spawn_ai(args: &BrowserArgs, workspace: &std::path::Path) -> Result<Option<CursorRelay>> {
    match resolve_ai_mode(args)? {
        AiMode::Off => Ok(None),
        AiMode::Cursor => {
            let relay = spawn_cursor(workspace)?;
            Ok(Some(relay))
        }
    }
}
