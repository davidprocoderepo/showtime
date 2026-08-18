//! Gerenciador de marcadores: CRUD, ordenação e busca.
//!
//! Thread-safety: a UI roda em uma única thread; o gerenciador é usado
//! diretamente pelo app e pelo modo ao vivo (leitura). Não compartilhar uma
//! instância entre threads sem `Mutex`.

use crate::markers::model::Marker;

/// Gerenciador de marcadores de um projeto.
#[derive(Debug, Default)]
pub struct MarkerManager {
    markers: Vec<Marker>,
    next_id: u32,
}

impl MarkerManager {
    pub fn new() -> Self {
        MarkerManager::default()
    }

    /// Cria um gerenciador a partir de uma lista existente, ajustando o
    /// contador de IDs para o maior ID + 1.
    pub fn with_markers(markers: Vec<Marker>) -> Self {
        let next_id = markers.iter().map(|m| m.id).max().map_or(1, |max| max + 1);
        MarkerManager { markers, next_id }
    }

    /// Lista ordenada por `time_sec` (ordem da timeline).
    pub fn markers(&self) -> &[Marker] {
        self.markers.as_slice()
    }

    /// Adiciona um marcador (o ID é atribuído automaticamente) e reordena por
    /// `time_sec`.
    pub fn add(
        &mut self,
        name: impl Into<String>,
        time_sec: f64,
        timecode: crate::timecode::Timecode,
        cue_number: u32,
        executor: u32,
        marker_type: crate::markers::model::MarkerType,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.markers.push(Marker::new(
            id,
            name,
            time_sec,
            timecode,
            cue_number,
            executor,
            marker_type,
        ));
        self.sort_by_time();
        id
    }

    /// Atualiza um marcador existente pelo ID. Retorna `false` se o ID não
    /// existir.
    pub fn update(&mut self, id: u32, f: impl FnOnce(&mut Marker)) -> bool {
        if let Some(marker) = self.markers.iter_mut().find(|m| m.id == id) {
            f(marker);
            self.sort_by_time();
            true
        } else {
            false
        }
    }

    /// Remove um marcador pelo ID. Retorna `false` se o ID não existir.
    pub fn remove(&mut self, id: u32) -> bool {
        let before = self.markers.len();
        self.markers.retain(|m| m.id != id);
        self.markers.len() != before
    }

    /// Busca um marcador pelo ID.
    pub fn get(&self, id: u32) -> Option<&Marker> {
        self.markers.iter().find(|m| m.id == id)
    }

    /// Ordena a lista por `time_sec` (estável, preservando IDs).
    pub fn sort_by_time(&mut self) {
        self.markers.sort_by(|a, b| {
            a.time_sec
                .partial_cmp(&b.time_sec)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Próximo ID livre (sem incrementar).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn peek_next_id(&self) -> u32 {
        self.next_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markers::model::MarkerType;
    use crate::timecode::Timecode;

    fn t(sec: f64) -> Timecode {
        // Timecode aproximado (não usado para aritmética nos testes do manager).
        let f = (sec * 30.0).round() as u32;
        Timecode {
            hours: f / 3600 / 30,
            minutes: (f / 30 / 60) % 60,
            seconds: (f / 30) % 60,
            frames: f % 30,
        }
    }

    #[test]
    fn add_assigns_ids_and_sorts() {
        let mut mgr = MarkerManager::new();
        let id1 = mgr.add("B", 5.0, t(5.0), 1, 1, MarkerType::Go);
        let id2 = mgr.add("A", 1.0, t(1.0), 2, 1, MarkerType::Pause);
        assert_ne!(id1, id2);
        let markers = mgr.markers();
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].name, "A");
        assert_eq!(markers[1].name, "B");
    }

    #[test]
    fn update_and_remove() {
        let mut mgr = MarkerManager::new();
        let id = mgr.add("X", 1.0, t(1.0), 1, 1, MarkerType::Go);
        assert!(mgr.update(id, |m| m.name = "Y".into()));
        assert_eq!(mgr.get(id).unwrap().name, "Y");
        assert!(mgr.remove(id));
        assert!(mgr.get(id).is_none());
        assert!(!mgr.remove(id));
    }

    #[test]
    fn with_markers_continues_ids() {
        let markers = vec![Marker::new(3, "A", 1.0, Timecode::ZERO, 1, 1, MarkerType::Go)];
        let mgr = MarkerManager::with_markers(markers);
        assert_eq!(mgr.peek_next_id(), 4);
    }
}