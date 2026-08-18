//! Export CSV: `timecode, cue_number, executor, tipo, nome, comentario`.
//!
//! Função pura: campos com vírgula/aspas são citados no padrão RFC 4180.

use crate::markers::model::Marker;

/// Formata um campo CSV: entre aspas se contiver vírgula, aspas ou quebra de
/// linha (aspas internas são duplicadas, conforme RFC 4180).
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Gera o CSV completo (cabeçalho + uma linha por marcador).
pub fn to_csv(markers: &[Marker]) -> String {
    let mut out = String::from("timecode,cue_number,executor,tipo,nome,comentario\n");
    for m in markers {
        let comment = m.comment.as_deref().unwrap_or("");
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            m.timecode,
            m.cue_number,
            m.executor,
            m.marker_type.as_str(),
            csv_field(&m.name),
            csv_field(comment),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markers::model::MarkerType;

    fn marker(name: &str, comment: Option<&str>) -> Marker {
        let mut m = Marker::new(1, name, 12.5, "00:00:12:15".parse().unwrap(), 3, 2, MarkerType::Go);
        m.comment = comment.map(Into::into);
        m
    }

    #[test]
    fn header_and_columns() {
        let csv = to_csv(&[marker("Intro", None)]);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "timecode,cue_number,executor,tipo,nome,comentario");
        assert_eq!(lines[1], "00:00:12:15,3,2,go,Intro,");
    }

    #[test]
    fn fields_with_comma_are_quoted() {
        let csv = to_csv(&[marker("Drop, main", Some("comentário com, vírgula"))]);
        assert!(csv.contains("\"Drop, main\""));
        assert!(csv.contains("\"comentário com, vírgula\""));
    }

    #[test]
    fn embedded_quotes_doubled() {
        let csv = to_csv(&[marker("A \"B\" C", None)]);
        assert!(csv.contains("\"A \"\"B\"\" C\""));
    }
}