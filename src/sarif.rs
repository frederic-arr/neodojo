use codespan_reporting::diagnostic::{Diagnostic, Label, Severity};
use codespan_reporting::files::{Files, SimpleFiles};
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use serde_sarif::sarif::{Region, Sarif};
use std::ops::Range;

fn try_get_byte_offset(
    file_id: usize,
    files: &SimpleFiles<String, String>,
    row: i64,
    column: i64,
) -> anyhow::Result<usize> {
    files
        .line_range(file_id, row as usize - 1)?
        .find(|byte| {
            if let Ok(location) = files.location(file_id, *byte) {
                location.column_number == column as usize
            } else {
                false
            }
        })
        .ok_or_else(|| anyhow::anyhow!("Byte offset not found"))
}

pub fn get_byte_range(
    file_id: usize,
    files: &SimpleFiles<String, String>,
    region: &Region,
) -> Range<usize> {
    // todo: support character regions
    let byte_offset = if let Some(byte_offset) = region.byte_offset {
        Some(byte_offset as usize)
    } else if let (Some(start_line), Some(start_column)) =
        (region.start_line, region.start_column.or(Some(1)))
    {
        if let Ok(byte_offset) = try_get_byte_offset(file_id, files, start_line, start_column) {
            Some(byte_offset)
        } else {
            None
        }
    } else {
        None
    };

    let byte_end = if let Some(byte_offset) = byte_offset {
        if let Some(byte_length) = region.byte_length {
            Some(byte_offset + byte_length as usize)
        } else if let (Some(end_line), Some(end_column)) = (
            region.end_line.map_or_else(|| region.start_line, Some), // if no end_line, default to start_line
            region.end_column.map_or_else(
                // if no end column use the line's last column
                || {
                    region
                        .end_line
                        .map_or_else(|| region.start_line, Some)
                        .and_then(|start_line| {
                            files
                                .line_range(file_id, start_line as usize - 1)
                                .map_or(None, Option::from)
                                .and_then(|byte_range| {
                                    byte_range.last().and_then(|last_byte| {
                                        files
                                            .column_number(
                                                file_id,
                                                start_line as usize - 1,
                                                last_byte,
                                            )
                                            .map_or(None, |v| Option::from(v as i64))
                                    })
                                })
                        })
                },
                Some,
            ),
        ) {
            if let Ok(byte_offset) = try_get_byte_offset(file_id, files, end_line, end_column) {
                Some(byte_offset)
            } else {
                Some(byte_offset)
            }
        } else {
            Some(byte_offset)
        }
    } else {
        None
    };

    byte_offset.unwrap_or_default()..byte_end.unwrap_or_default()
}

type BuildDiagnosticVec = Vec<(SimpleFiles<String, String>, Vec<Diagnostic<usize>>)>;

#[derive(Debug, Clone, Default)]
pub struct BuildDiagnostic(BuildDiagnosticVec);

impl From<Sarif> for BuildDiagnostic {
    fn from(sarif: Sarif) -> Self {
        let mut diagnostics = Vec::new();
        for run in &sarif.runs {
            let mut files_map = std::collections::HashMap::new();
            let mut files = SimpleFiles::new();
            let mut run_diagnostics = Vec::new();
            for artifact in run.artifacts.as_ref().unwrap().iter() {
                let location = artifact.location.as_ref().unwrap();
                let name = location.uri.as_ref().unwrap().to_string();
                let parent = location.uri_base_id.as_ref().unwrap().to_string();

                let content = artifact.contents.as_ref().unwrap().text.as_ref().unwrap();
                let id = files.add(name.clone(), content.clone());
                files_map.insert((parent, name), id);
            }

            for result in run.results.as_ref().unwrap() {
                let level = result
                    .level
                    .clone()
                    .unwrap_or(serde_json::Value::Null)
                    .as_str()
                    .unwrap_or("error")
                    .to_string();

                let message = result.message.text.as_ref().unwrap().to_string();

                let location = result.locations.as_ref().unwrap()[0]
                    .physical_location
                    .as_ref()
                    .unwrap()
                    .artifact_location
                    .as_ref()
                    .unwrap();
                let name = location.uri.as_ref().unwrap().to_string();
                let parent = location.uri_base_id.as_ref().unwrap().to_string();
                let file_id = *files_map.get(&(parent, name)).unwrap();
                let region = result.locations.as_ref().unwrap()[0]
                    .physical_location
                    .as_ref()
                    .unwrap()
                    .region
                    .as_ref()
                    .unwrap();

                let range = get_byte_range(file_id, &files, region);
                let diagnostic: Diagnostic<usize> = match level.as_str() {
                    "error" => Diagnostic::error(),
                    "warning" => Diagnostic::warning(),
                    _ => Diagnostic::note(),
                };

                let diagnostic = diagnostic
                    .with_message(message.clone())
                    .with_labels(vec![Label::primary(file_id, range).with_message(message)]);

                run_diagnostics.push(diagnostic);
            }

            diagnostics.push((files, run_diagnostics));
        }

        Self(diagnostics)
    }
}

impl From<BuildDiagnosticVec> for BuildDiagnostic {
    fn from(diagnostics: BuildDiagnosticVec) -> Self {
        Self(diagnostics)
    }
}

impl BuildDiagnostic {
    pub fn has_errors(&self) -> bool {
        self.0.iter().any(|(_, diagnostics)| {
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error)
        })
    }

    pub fn pretty_print(&self) {
        let writer = StandardStream::stdout(ColorChoice::Auto);
        let config = codespan_reporting::term::Config::default();
        for (files, diagnostics) in &self.0 {
            let mut diagnostics = diagnostics.clone();
            diagnostics.sort_by(|a, b| a.severity.partial_cmp(&b.severity).unwrap());
            for diagnostic in &diagnostics {
                codespan_reporting::term::emit(&mut writer.lock(), &config, files, diagnostic)
                    .unwrap();
            }
        }
    }
}

impl std::ops::AddAssign for BuildDiagnostic {
    fn add_assign(&mut self, other: Self) {
        self.0.extend(other.0);
    }
}
