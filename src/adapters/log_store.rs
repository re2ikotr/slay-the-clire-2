use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::log::LogEntry;
use crate::core::StepResult;

const LOG_DIR: &str = "logs";

pub struct StepLogSink {
    path: PathBuf,
    file: File,
    step: u64,
}

impl StepLogSink {
    pub fn create(session: &str) -> io::Result<Self> {
        create_dir_all(LOG_DIR)?;

        let created_ms = unix_time_millis();
        let file_name = format!(
            "{}-{}-{}.log",
            filename_fragment(session),
            created_ms,
            std::process::id()
        );
        let path = PathBuf::from(LOG_DIR).join(file_name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;

        writeln!(file, "session: {session}")?;
        writeln!(file, "created_unix_ms: {created_ms}")?;
        writeln!(file)?;

        Ok(Self {
            path,
            file,
            step: 0,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn record_step(&mut self, label: &str, result: &StepResult) -> io::Result<()> {
        self.step += 1;
        let entries = step_entries(result);

        writeln!(self.file, "step: {}", self.step)?;
        writeln!(self.file, "label: {label}")?;
        writeln!(self.file, "result: {}", result_name(result))?;
        writeln!(self.file, "entries: {}", entries.len())?;
        match result {
            StepResult::NeedChoice(choice, _) => writeln!(self.file, "choice: {choice:?}")?,
            StepResult::CombatOver(result, _) => writeln!(self.file, "combat_result: {result:?}")?,
            StepResult::Rejected(error, _) => writeln!(self.file, "error: {error}")?,
            StepResult::Failed(error, _) => writeln!(self.file, "error: {error}")?,
            StepResult::Done(_) => {}
        }

        for (index, entry) in entries.iter().enumerate() {
            writeln!(self.file, "{index:04}: {entry:?}")?;
        }
        writeln!(self.file)?;
        self.file.flush()
    }

    pub fn record_note(&mut self, label: &str, note: &str) -> io::Result<()> {
        writeln!(self.file, "note: {label}")?;
        writeln!(self.file, "{note}")?;
        writeln!(self.file)?;
        self.file.flush()
    }
}

fn step_entries(result: &StepResult) -> &[LogEntry] {
    match result {
        StepResult::Done(log)
        | StepResult::NeedChoice(_, log)
        | StepResult::CombatOver(_, log)
        | StepResult::Rejected(_, log)
        | StepResult::Failed(_, log) => log,
    }
}

fn result_name(result: &StepResult) -> &'static str {
    match result {
        StepResult::Done(_) => "done",
        StepResult::NeedChoice(_, _) => "need_choice",
        StepResult::CombatOver(_, _) => "combat_over",
        StepResult::Rejected(_, _) => "rejected",
        StepResult::Failed(_, _) => "failed",
    }
}

fn unix_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn filename_fragment(input: &str) -> String {
    let fragment = input
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if fragment.is_empty() {
        "session".to_string()
    } else {
        fragment
    }
}
