//! Script/macro de texto para GrandMA2.
//!
//! Linhas de comando por marcador:
//! - `Store Executor N Cue N` — cria a cue no executor
//! - `Assign Timecode HH:MM:SS:FF Executor N Cue N` — associa o timecode
//!
//! Além do script de texto, gera o XML de macro
//! (`<Macro><MacroLine command=../></Macro>`), que é o formato de importação
//! no console (Setup → Import/Export → Import → Macro).

use crate::markers::model::Marker;

use super::xml::xml_escape;

/// Gera os comandos MA2 (uma linha por comando) para todos os marcadores.
pub fn to_ma2_commands(markers: &[Marker]) -> String {
    let mut out = String::new();
    for m in markers {
        out.push_str(&format!("Store Executor {} Cue {}\n", m.executor, m.cue_number));
        out.push_str(&format!(
            "Assign Timecode {} Executor {} Cue {}\n",
            m.timecode, m.executor, m.cue_number
        ));
    }
    out
}

/// Gera um arquivo `.xml` de macro MA2 a partir de uma lista de comandos.
///
/// Cada linha do comando vira um `<MacroLine command="..."/>`. Os params
/// `Wait`/`Info`/`Disabled` ficam no padrão (0/""/false); se o console exportar
/// um DTD diferente, ajuste aqui.
pub fn to_macro_xml(macro_name: &str, commands: &[String]) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!("<Macro name=\"{}\">\n", xml_escape(macro_name)));
    for cmd in commands {
        out.push_str(&format!("  <MacroLine command=\"{}\"/>\n", xml_escape(cmd)));
    }
    out.push_str("</Macro>\n");
    out
}

/// Conveniência: comando de macro MA2 a partir dos marcadores.
pub fn to_ma2_macro_xml(macro_name: &str, markers: &[Marker]) -> String {
    let commands: Vec<String> = to_ma2_commands(markers)
        .lines()
        .map(str::to_string)
        .collect();
    to_macro_xml(macro_name, &commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markers::model::MarkerType;

    fn marker() -> Marker {
        Marker::new(1, "Intro", 12.5, "00:00:12:15".parse().unwrap(), 3, 2, MarkerType::Go)
    }

    #[test]
    fn commands_store_and_assign() {
        let script = to_ma2_commands(&[marker()]);
        assert_eq!(
            script,
            "Store Executor 2 Cue 3\nAssign Timecode 00:00:12:15 Executor 2 Cue 3\n"
        );
    }

    #[test]
    fn macro_xml_structure() {
        let xml = to_ma2_macro_xml("Showtime Cues", &[marker()]);
        assert!(xml.contains("<Macro name=\"Showtime Cues\">"));
        assert!(xml.contains("<MacroLine command=\"Store Executor 2 Cue 3\"/>"));
        assert!(xml.contains("<MacroLine command=\"Assign Timecode 00:00:12:15 Executor 2 Cue 3\"/>"));
        assert!(xml.contains("</Macro>"));
    }

    #[test]
    fn macro_xml_escapes_commands() {
        let xml = to_macro_xml("M & M", &["Store Executor 1 Cue <2>".into()]);
        assert!(xml.contains("<Macro name=\"M &amp; M\">"));
        assert!(xml.contains("command=\"Store Executor 1 Cue &lt;2&gt;\""));
    }
}