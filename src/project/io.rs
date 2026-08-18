//! Salvar/carregar projetos em JSON ou YAML.
//!
//! YAML usa `yaml_serde` (fork oficial mantido do `serde_yaml`). NUNCA usar
//! o crate `serde_yaml` original (arquivado) nem `serde_yml` (RUSTSEC-2025-0068).

use std::path::Path;

use crate::error::ShowtimeError;
use crate::project::model::Project;

/// Salva o projeto em JSON.
pub fn save_json(project: &Project, path: &Path) -> Result<(), ShowtimeError> {
    let json = serde_json::to_string_pretty(project)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Carrega um projeto de JSON.
pub fn load_json(path: &Path) -> Result<Project, ShowtimeError> {
    let data = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

/// Salva o projeto em YAML.
pub fn save_yaml(project: &Project, path: &Path) -> Result<(), ShowtimeError> {
    let yaml = yaml_serde::to_string(project)
        .map_err(ShowtimeError::SerdeYaml)?;
    std::fs::write(path, yaml)?;
    Ok(())
}

/// Carrega um projeto de YAML.
pub fn load_yaml(path: &Path) -> Result<Project, ShowtimeError> {
    let data = std::fs::read_to_string(path)?;
    yaml_serde::from_str(&data).map_err(ShowtimeError::SerdeYaml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markers::model::MarkerType;
    use crate::timecode::Timecode;

    fn sample() -> Project {
        Project {
            name: "Show 1".into(),
            audio_file_path: Some("musica.mp3".into()),
            frame_rate: 29.97,
            drop_frame: true,
            timecode_offset: "01:00:00:00".parse().unwrap(),
            markers: vec![
                crate::markers::model::Marker::new(
                    1,
                    "Intro",
                    0.0,
                    Timecode::ZERO,
                    1,
                    1,
                    MarkerType::Go,
                ),
                crate::markers::model::Marker::new(
                    2,
                    "Drop",
                    123.4,
                    "01:02:03:04".parse().unwrap(),
                    2,
                    2,
                    MarkerType::Toggle,
                ),
            ],
        }
    }

    #[test]
    fn json_roundtrip() {
        let dir = std::env::temp_dir().join(format!("showtime-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("proj.json");
        let p = sample();
        save_json(&p, &path).unwrap();
        let loaded = load_json(&path).unwrap();
        assert_eq!(loaded.name, p.name);
        assert_eq!(loaded.markers.len(), 2);
        assert_eq!(loaded.markers[1].timecode.to_string(), "01:02:03:04");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn yaml_roundtrip() {
        let dir = std::env::temp_dir().join(format!("showtime-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("proj.yaml");
        let p = sample();
        save_yaml(&p, &path).unwrap();
        let loaded = load_yaml(&path).unwrap();
        assert_eq!(loaded.name, p.name);
        assert!(loaded.drop_frame);
        assert_eq!(loaded.timecode_offset.to_string(), "01:00:00:00");
        let _ = std::fs::remove_dir_all(&dir);
    }
}