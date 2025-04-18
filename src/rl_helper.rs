use std::borrow::Cow;
use std::borrow::Cow::{Borrowed, Owned};
use colored::Colorize;
use rustyline::{Cmd, Completer, CompletionType, Config, EditMode, Editor, Helper, Hinter, KeyEvent, Validator};
use rustyline::completion::FilenameCompleter;
use rustyline::highlight::{CmdKind, Highlighter, MatchingBracketHighlighter};
use rustyline::hint::HistoryHinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::MatchingBracketValidator;

#[derive(Helper, Completer, Hinter, Validator)]
pub struct RlHelper {
    #[rustyline(Completer)]
    completer: FilenameCompleter,
    #[rustyline(Highlighter)]
    highlighter: MatchingBracketHighlighter,
    #[rustyline(Validator)]
    validator: MatchingBracketValidator,
    #[rustyline(Hinter)]
    hinter: HistoryHinter,
    colored_prompt: String,
}

impl Highlighter for RlHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(&'s self, prompt: &'p str, default: bool) -> Cow<'b, str> {
        if default { Borrowed(&self.colored_prompt) } else { Borrowed(prompt) }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned("\x1b[1m".to_owned() + hint + "\x1b[m")
    }

    fn highlight_char(&self, line: &str, pos: usize, kind: CmdKind) -> bool {
        self.highlighter.highlight_char(line, pos, kind)
    }
}

impl RlHelper {
    pub fn new_rl() -> anyhow::Result<Editor<RlHelper, DefaultHistory>> {
        let config = Config::builder()
            .history_ignore_space(true)
            .completion_type(CompletionType::List)
            .edit_mode(EditMode::Emacs)
            .build();

        let helper = Self {
            completer: FilenameCompleter::new(),
            highlighter: MatchingBracketHighlighter::new(),
            hinter: HistoryHinter::new(),
            colored_prompt: "".to_owned(),
            validator: MatchingBracketValidator::new(),
        };

        let mut rl = Editor::with_config(config)?;
        rl.set_helper(Some(helper));
        rl.bind_sequence(KeyEvent::alt('n'), Cmd::HistorySearchForward);
        rl.bind_sequence(KeyEvent::alt('p'), Cmd::HistorySearchBackward);
        let _ = rl.load_history("_history.txt");
        
        rl.helper_mut().expect("No helper found").colored_prompt = "🌟 ^D:".blue().to_string();
        Ok(rl)
    }
}
