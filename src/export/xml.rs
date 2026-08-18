//! Export XML genérico de marcadores.
//!
//! Função pura; campos com caracteres especiais XML são escapados.

use crate::markers::model::Marker;

/// Escapa texto para uso como conteúdo de elemento XML.
pub(crate) fn xml_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            _ => c.to_string(),
        })
        .collect()
}

/// Gera o XML na estrutura:
/// `<markers><marker><timecode/><cue/><executor/><type/><name/><comment/></marker></markers>`
pub fn to_xml(markers: &[Marker]) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<markers>\n");
    for m in markers {
        out.push_str("  <marker>\n");
        out.push_str(&format!("    <timecode>{}</timecode>\n", m.timecode));
        out.push_str(&format!("    <cue>{}</cue>\n", m.cue_number));
        out.push_str(&format!("    <executor>{}</executor>\n", m.executor));
        out.push_str(&format!("    <type>{}</type>\n", m.marker_type.as_str()));
        out.push_str(&format!("    <name>{}</name>\n", xml_escape(&m.name)));
        out.push_str(&format!(
            "    <comment>{}</comment>\n",
            xml_escape(m.comment.as_deref().unwrap_or(""))
        ));
        out.push_str("  </marker>\n");
    }
    out.push_str("</markers>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markers::model::MarkerType;

    fn marker() -> Marker {
        Marker::new(1, "Intro", 12.5, "00:00:12:15".parse().unwrap(), 3, 2, MarkerType::Go)
    }

    #[test]
    fn structure_and_columns() {
        let xml = to_xml(&[marker()]);
        assert!(xml.contains("<markers>"));
        assert!(xml.contains("<timecode>00:00:12:15</timecode>"));
        assert!(xml.contains("<cue>3</cue>"));
        assert!(xml.contains("<executor>2</executor>"));
        assert!(xml.contains("<type>go</type>"));
        assert!(xml.contains("<name>Intro</name>"));
        assert!(xml.contains("<comment></comment>"));
        assert!(xml.contains("</markers>"));
    }

    #[test]
    fn special_chars_escaped() {
        let mut m = marker();
        m.name = "A & B <C>".into();
        m.comment = Some("val \"aspas\"".into());
        let xml = to_xml(&[m]);
        assert!(xml.contains("<name>A &amp; B &lt;C&gt;</name>"));
        assert!(xml.contains("<comment>val &quot;aspas&quot;</comment>"));
    }

    #[test]
    fn xml_escape_covers_all() {
        assert_eq!(xml_escape("&<>\"'"), "&amp;&lt;&gt;&quot;&apos;");
    }
}