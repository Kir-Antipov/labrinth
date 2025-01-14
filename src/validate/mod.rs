use crate::models::projects::{GameVersion, Loader};
use crate::validate::fabric::FabricValidator;
use crate::validate::forge::{ForgeValidator, LegacyForgeValidator};
use crate::validate::pack::PackValidator;
use chrono::{DateTime, Utc};
use std::io::Cursor;
use thiserror::Error;
use zip::ZipArchive;

mod fabric;
mod forge;
mod pack;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Unable to read Zip Archive: {0}")]
    ZipError(#[from] zip::result::ZipError),
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Error while validating JSON: {0}")]
    SerDeError(#[from] serde_json::Error),
    #[error("Invalid Input: {0}")]
    InvalidInputError(String),
}

#[derive(Eq, PartialEq)]
pub enum ValidationResult {
    /// File should be marked as primary
    Pass,
    /// File should not be marked primary, the reason for which is inside the String
    Warning(String),
}

pub enum SupportedGameVersions {
    All,
    PastDate(DateTime<Utc>),
    Range(DateTime<Utc>, DateTime<Utc>),
    Custom(Vec<GameVersion>),
}

pub trait Validator: Sync {
    fn get_file_extensions<'a>(&self) -> &'a [&'a str];
    fn get_project_types<'a>(&self) -> &'a [&'a str];
    fn get_supported_loaders<'a>(&self) -> &'a [&'a str];
    fn get_supported_game_versions(&self) -> SupportedGameVersions;
    fn validate(
        &self,
        archive: &mut ZipArchive<Cursor<&[u8]>>,
    ) -> Result<ValidationResult, ValidationError>;
}

static VALIDATORS: [&dyn Validator; 4] = [
    &PackValidator {},
    &FabricValidator {},
    &ForgeValidator {},
    &LegacyForgeValidator {},
];

/// The return value is whether this file should be marked as primary or not, based on the analysis of the file
pub fn validate_file(
    data: &[u8],
    file_extension: &str,
    project_type: &str,
    loaders: Vec<Loader>,
    game_versions: Vec<GameVersion>,
    all_game_versions: &[crate::database::models::categories::GameVersion],
) -> Result<ValidationResult, ValidationError> {
    let reader = std::io::Cursor::new(data);
    let mut zip = zip::ZipArchive::new(reader)?;

    let mut visited = false;
    for validator in &VALIDATORS {
        if validator.get_project_types().contains(&project_type)
            && loaders
                .iter()
                .any(|x| validator.get_supported_loaders().contains(&&*x.0))
            && game_version_supported(
                &game_versions,
                all_game_versions,
                validator.get_supported_game_versions(),
            )
        {
            if validator.get_file_extensions().contains(&file_extension) {
                return validator.validate(&mut zip);
            } else {
                visited = true;
            }
        }
    }

    if visited {
        Err(ValidationError::InvalidInputError(format!(
            "File extension {} is invalid for input file",
            file_extension
        )))
    } else {
        Ok(ValidationResult::Pass)
    }
}

fn game_version_supported(
    game_versions: &[GameVersion],
    all_game_versions: &[crate::database::models::categories::GameVersion],
    supported_game_versions: SupportedGameVersions,
) -> bool {
    match supported_game_versions {
        SupportedGameVersions::All => true,
        SupportedGameVersions::PastDate(date) => game_versions.iter().any(|x| {
            all_game_versions
                .iter()
                .find(|y| y.version == x.0)
                .map(|x| x.date > date)
                .unwrap_or(false)
        }),
        SupportedGameVersions::Range(before, after) => game_versions.iter().any(|x| {
            all_game_versions
                .iter()
                .find(|y| y.version == x.0)
                .map(|x| x.date > before && x.date < after)
                .unwrap_or(false)
        }),
        SupportedGameVersions::Custom(versions) => {
            versions.iter().any(|x| game_versions.contains(x))
        }
    }
}
