//! Geração e cache de waveform (picos min/max por bloco).
//!
//! Função pura e testável: downmix mono antes dos picos; cada bloco de
//! `block_size` amostras (mono) vira um par (min, max) normalizado em
//! [-1.0, 1.0].

/// Waveform com picos min/max por bloco.
#[derive(Debug, Clone)]
pub struct Waveform {
    /// Pares (min, max) por bloco, na ordem da timeline.
    pub peaks: Vec<(f32, f32)>,
    /// Tamanho do bloco em amostras mono.
    pub block_size: usize,
}

/// Calcula os picos min/max por bloco de `block_size` amostras.
///
/// * `samples` — PCM interleaved (L,R,L,R,...).
/// * `block_size` — nº de amostras mono por bloco (ex.: 1024).
/// * `channels` — nº de canais (downmix mono por frame antes dos picos).
pub fn compute_peaks(samples: &[f32], block_size: usize, channels: u16) -> Waveform {
    if block_size == 0 || channels == 0 || samples.is_empty() {
        return Waveform {
            peaks: Vec::new(),
            block_size,
        };
    }
    let ch = channels as usize;
    let frames = samples.len() / ch;
    let mut peaks = Vec::with_capacity(frames / block_size + 1);

    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut count = 0usize;

    for frame in 0..frames {
        let start = frame * ch;
        let mono: f32 = samples[start..start + ch].iter().sum::<f32>() / ch as f32;
        min = min.min(mono);
        max = max.max(mono);
        count += 1;
        if count >= block_size {
            peaks.push((min, max));
            min = f32::INFINITY;
            max = f32::NEG_INFINITY;
            count = 0;
        }
    }
    // Bloco final parcial.
    if count > 0 {
        peaks.push((min, max));
    }

    // Normaliza para [-1.0, 1.0] se houver overflow (amostras fora da faixa).
    let scale = peaks
        .iter()
        .flat_map(|&(lo, hi)| [lo.abs(), hi.abs()])
        .fold(0.0f32, f32::max);
    if scale > 1.0 {
        for p in &mut peaks {
            p.0 /= scale;
            p.1 /= scale;
        }
    }

    Waveform { peaks, block_size }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_gives_empty_peaks() {
        assert!(compute_peaks(&[], 1024, 2).peaks.is_empty());
        assert!(compute_peaks(&[0.5], 0, 2).peaks.is_empty());
    }

    #[test]
    fn mono_block_counts() {
        // 44100 amostras mono, bloco 1024 -> 43 blocos cheios + 1 parcial.
        let samples: Vec<f32> = (0..44100).map(|i| ((i % 100) as f32 / 50.0) - 1.0).collect();
        let w = compute_peaks(&samples, 1024, 1);
        assert_eq!(w.peaks.len(), 44);
        assert_eq!(w.block_size, 1024);
    }

    #[test]
    fn downmix_stereo_before_peaks() {
        // L = R = constante: o mono por frame é a própria constante.
        let samples: Vec<f32> = (0..44100 * 2)
            .map(|_| 0.5)
            .collect();
        let w = compute_peaks(&samples, 1024, 2);
        for &(lo, hi) in &w.peaks {
            assert_eq!(lo, 0.5);
            assert_eq!(hi, 0.5);
        }
    }

    #[test]
    fn peaks_contain_min_and_max() {
        // Sinal com picos extremos dentro de um mesmo bloco.
        let mut samples: Vec<f32> = vec![0.0; 2048];
        samples[0] = -1.0;
        samples[1023] = 1.0;
        let w = compute_peaks(&samples, 1024, 1);
        assert_eq!(w.peaks[0], (-1.0, 1.0));
    }

    #[test]
    fn normalization_clamps_overflow() {
        let mut samples: Vec<f32> = vec![0.0; 1024];
        samples[0] = 2.0;
        samples[1] = -2.0;
        let w = compute_peaks(&samples, 1024, 1);
        assert!(w.peaks[0].0 >= -1.0 && w.peaks[0].1 <= 1.0);
    }
}