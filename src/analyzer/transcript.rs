use serde::{Deserialize, Serialize};

fn default_source() -> String {
    "generated".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub language: String,
    pub segments: Vec<Segment>,
    #[serde(default = "default_source")]
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Word {
    pub word: String,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub estimated: bool,
}

impl Transcript {
    pub fn load(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let data = std::fs::read_to_string(path)?;
        let transcript: Transcript = serde_json::from_str(&data)?;
        Ok(transcript)
    }

    pub fn split_long_segments(&mut self, max_words: usize) {
        let mut new_segments = Vec::new();
        for seg in &self.segments {
            if seg.words.len() <= max_words {
                new_segments.push(seg.clone());
                continue;
            }
            for chunk in seg.words.chunks(max_words) {
                let text = chunk.iter().map(|w| w.word.as_str()).collect::<Vec<_>>().join(" ");
                new_segments.push(Segment {
                    text,
                    start: chunk.first().unwrap().start,
                    end: chunk.last().unwrap().end,
                    words: chunk.to_vec(),
                });
            }
        }
        self.segments = new_segments;
    }
}
