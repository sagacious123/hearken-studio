//! Search by meaning: find verses that say what someone said in their own words ("don't worry
//! about tomorrow" → Matthew 6:34), when the words themselves don't match.
//!
//! Uses Model2Vec's potion-base-8M (MIT licence, 30 MB, downloaded on request): a table of word-
//! piece vectors distilled from a sentence-embedding model. A sentence's vector is the mean of
//! its pieces' vectors, so there's no neural network to run: embedding all 31,000 verses takes
//! well under a second, and a search compares one vector against them all in a few
//! milliseconds, on any computer, offline.

use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use super::{db::BibleDb, ScriptureRef};

const REVISION: &str = "bf8b056651a2c21b8d2565580b8569da283cab23";
/// (file, bytes, sha256), pinned to one revision of minishlab/potion-base-8M.
const FILES: [(&str, u64, &str); 2] = [
    ("model.safetensors", 30_236_760, "f65d0f325faadc1e121c319e2faa41170d3fa07d8c89abd48ca5358d9a223de2"),
    ("vocab.txt", 219_690, "1394523a67ddd404a825428018c0582a6998bcfa044ecbcbf1f4d71adb94c61c"),
];
pub const DOWNLOAD_MB: f32 = 30.5;

/// Verses are indexed in modern English, which is how people paraphrase.
pub const INDEX_TRANSLATION: &str = "WEB";

pub fn model_dir(models_dir: &Path) -> PathBuf {
    models_dir.join("potion-base-8M")
}

pub fn installed(models_dir: &Path) -> bool {
    let dir = model_dir(models_dir);
    FILES.iter().all(|(f, bytes, _)| fs::metadata(dir.join(f)).is_ok_and(|m| m.len() == *bytes))
}

/// Download the model (checksum-verified). `progress` gets bytes received and the total.
pub fn download(models_dir: &Path, progress: impl Fn(u64, u64)) -> Result<()> {
    let dir = model_dir(models_dir);
    fs::create_dir_all(&dir)?;
    let total: u64 = FILES.iter().map(|f| f.1).sum();
    let mut done = 0;
    for (file, bytes, sha) in FILES {
        let dest = dir.join(file);
        if fs::metadata(&dest).is_ok_and(|m| m.len() == bytes) {
            done += bytes;
            continue;
        }
        let url = format!("https://huggingface.co/minishlab/potion-base-8M/resolve/{REVISION}/{file}");
        let resp = ureq::get(&url).timeout(std::time::Duration::from_secs(300)).call().context("couldn't download the meaning model. Is this computer online?")?;
        let mut reader = resp.into_reader().take(bytes + 1);
        let part = dir.join(format!("{file}.part"));
        let mut out = fs::File::create(&part)?;
        let mut hash = Sha256::new();
        let mut buf = vec![0u8; 256 * 1024];
        let mut got = 0;
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            hash.update(&buf[..n]);
            got += n as u64;
            progress(done + got, total);
        }
        drop(out);
        let digest: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if got != bytes || digest != sha {
            let _ = fs::remove_file(&part);
            bail!("the downloaded {file} didn't match its checksum; please try again");
        }
        fs::rename(&part, &dest)?;
        done += bytes;
    }
    Ok(())
}

pub fn remove(models_dir: &Path) -> Result<()> {
    let dir = model_dir(models_dir);
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}

pub struct Model {
    vocab: HashMap<String, u32>,
    unk: Option<u32>,
    /// rows × dim, row-major.
    table: Vec<f32>,
    dim: usize,
}

impl Model {
    pub fn load(models_dir: &Path) -> Result<Self> {
        let dir = model_dir(models_dir);
        let vocab_text = fs::read_to_string(dir.join("vocab.txt")).context("reading the meaning model's vocabulary")?;
        let vocab: HashMap<String, u32> = vocab_text.lines().enumerate().map(|(i, t)| (t.to_owned(), i as u32)).collect();
        let (table, rows, dim) = read_safetensors(&fs::read(dir.join("model.safetensors")).context("reading the meaning model")?)?;
        if rows != vocab.len() {
            bail!("meaning model and vocabulary don't match ({rows} vs {})", vocab.len());
        }
        Ok(Self { unk: vocab.get("[UNK]").copied(), vocab, table, dim })
    }

    /// BERT word pieces: lowercase, accents dropped, punctuation split off, then the longest
    /// known pieces ("forgiveness" → "forgive", "##ness").
    fn tokens(&self, text: &str) -> Vec<u32> {
        let mut out = Vec::new();
        for word in pre_tokenize(&normalize(text)) {
            if word.chars().count() > 100 {
                out.extend(self.unk);
                continue;
            }
            let chars: Vec<char> = word.chars().collect();
            let mut start = 0;
            let mut pieces = Vec::new();
            while start < chars.len() {
                let mut end = chars.len();
                let mut found = None;
                while start < end {
                    let piece: String = chars[start..end].iter().collect();
                    let key = if start > 0 { format!("##{piece}") } else { piece };
                    if let Some(&id) = self.vocab.get(&key) {
                        found = Some(id);
                        break;
                    }
                    end -= 1;
                }
                match found {
                    Some(id) => {
                        pieces.push(id);
                        start = end;
                    }
                    None => {
                        pieces.clear();
                        pieces.extend(self.unk);
                        break;
                    }
                }
            }
            out.extend(pieces);
        }
        out
    }

    /// Unit-length sentence vector; `None` when no word is known.
    pub fn embed(&self, text: &str) -> Option<Vec<f32>> {
        let ids: Vec<u32> = self.tokens(text).into_iter().filter(|&id| Some(id) != self.unk).take(512).collect();
        if ids.is_empty() {
            return None;
        }
        let mut v = vec![0f32; self.dim];
        for id in &ids {
            let row = &self.table[*id as usize * self.dim..][..self.dim];
            v.iter_mut().zip(row).for_each(|(a, b)| *a += b);
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        (norm > 0.0).then(|| v.into_iter().map(|x| x / norm).collect())
    }
}

/// The one-tensor safetensors file Model2Vec writes: 8-byte header length, JSON header, data.
fn read_safetensors(bytes: &[u8]) -> Result<(Vec<f32>, usize, usize)> {
    let header_len = u64::from_le_bytes(bytes.get(..8).context("model file too short")?.try_into()?) as usize;
    let header: serde_json::Value = serde_json::from_slice(bytes.get(8..8 + header_len).context("bad model header")?)?;
    let t = &header["embeddings"];
    if t["dtype"] != "F32" {
        bail!("unsupported model format");
    }
    let shape: Vec<usize> = serde_json::from_value(t["shape"].clone())?;
    let offsets: Vec<usize> = serde_json::from_value(t["data_offsets"].clone())?;
    let [rows, dim] = shape[..] else { bail!("unexpected model shape") };
    let data = bytes.get(8 + header_len + offsets[0]..8 + header_len + offsets[1]).context("model data truncated")?;
    if data.len() != rows * dim * 4 {
        bail!("model data has the wrong size");
    }
    Ok((data.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect(), rows, dim))
}

/// BertNormalizer: control characters removed, CJK characters spaced, lowercased, accents
/// dropped (the common Latin ones; sermons are English).
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '\0' || c == '\u{fffd}' || (c.is_control() && !c.is_whitespace()) {
            continue;
        }
        if matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF) {
            out.push(' ');
            out.push(c);
            out.push(' ');
            continue;
        }
        for l in c.to_lowercase() {
            out.push(match l {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
                'è' | 'é' | 'ê' | 'ë' => 'e',
                'ì' | 'í' | 'î' | 'ï' => 'i',
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
                'ù' | 'ú' | 'û' | 'ü' => 'u',
                'ç' => 'c',
                'ñ' => 'n',
                '’' | '‘' => '\'',
                c => c,
            });
        }
    }
    out
}

/// BertPreTokenizer: split on whitespace, and every punctuation mark is its own token.
fn pre_tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split_whitespace() {
        let mut cur = String::new();
        for c in word.chars() {
            if c.is_ascii_punctuation() || is_unicode_punct(c) {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                out.push(c.to_string());
            } else {
                cur.push(c);
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

fn is_unicode_punct(c: char) -> bool {
    matches!(c, '“' | '”' | '—' | '–' | '…' | '«' | '»' | '¿' | '¡' | '·')
}

/// Every verse of the index translation as a vector, for nearest-neighbour search.
pub struct VerseIndex {
    refs: Vec<ScriptureRef>,
    vectors: Vec<f32>,
    dim: usize,
}

#[derive(Debug, Clone)]
pub struct MeaningHit {
    pub reference: ScriptureRef,
    /// Cosine similarity, 0–1.
    pub score: f32,
}

impl VerseIndex {
    pub fn build(model: &Model, db: &BibleDb) -> Result<Self> {
        let verses = db.all_verses(INDEX_TRANSLATION)?;
        let mut refs = Vec::with_capacity(verses.len());
        let mut vectors = Vec::with_capacity(verses.len() * model.dim);
        for (r, text) in verses {
            if let Some(v) = model.embed(&text) {
                refs.push(r);
                vectors.extend(v);
            }
        }
        Ok(Self { refs, vectors, dim: model.dim })
    }

    pub fn len(&self) -> usize {
        self.refs.len()
    }

    /// The `k` verses closest in meaning to `query`, best first.
    pub fn search(&self, model: &Model, query: &str, k: usize) -> Vec<MeaningHit> {
        let Some(q) = model.embed(query) else { return Vec::new() };
        let mut best: Vec<(f32, usize)> = Vec::with_capacity(k + 1);
        for (i, v) in self.vectors.chunks_exact(self.dim).enumerate() {
            let score: f32 = v.iter().zip(&q).map(|(a, b)| a * b).sum();
            if best.len() < k || score > best[best.len() - 1].0 {
                let pos = best.partition_point(|(s, _)| *s >= score);
                best.insert(pos, (score, i));
                best.truncate(k);
            }
        }
        best.into_iter().map(|(score, i)| MeaningHit { reference: self.refs[i].clone(), score }).collect()
    }
}

/// Live suggestions need this similarity: paraphrases of a verse score 0.75–0.9, ordinary
/// sermon sentences up to ~0.7 (measured on test sentences).
pub const LIVE_MIN_SCORE: f32 = 0.8;
/// Shorter sentences match too much by accident.
const LIVE_MIN_WORDS: usize = 6;
/// The search box shows weaker matches too, since the operator chooses.
pub const SEARCH_MIN_SCORE: f32 = 0.55;

/// The model with its verse index, ready to search.
pub struct Meaning {
    model: Model,
    index: VerseIndex,
}

impl Meaning {
    pub fn load(models_dir: &Path, db: &BibleDb) -> Result<Self> {
        let t = std::time::Instant::now();
        let model = Model::load(models_dir)?;
        let index = VerseIndex::build(&model, db)?;
        log::info!("search by meaning ready: {} verses in {:?}", index.len(), t.elapsed());
        Ok(Self { model, index })
    }

    pub fn search(&self, query: &str, k: usize) -> Vec<MeaningHit> {
        self.index.search(&self.model, query, k)
    }

    /// A finished sentence that says what a verse says, if one is close enough to suggest.
    pub fn live_hit(&self, sentence: &str) -> Option<MeaningHit> {
        if sentence.split_whitespace().count() < LIVE_MIN_WORDS {
            return None;
        }
        self.search(sentence, 1).into_iter().next().filter(|h| h.score >= LIVE_MIN_SCORE)
    }
}

/// How sure a live suggestion is, on the detection scale (never enough to project by itself).
pub fn confidence(score: f32) -> f32 {
    (0.45 + (score - LIVE_MIN_SCORE) * 2.0).clamp(0.35, 0.8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_splits_like_bert() {
        assert_eq!(normalize("Don’t WORRY, café"), "don't worry, cafe");
        assert_eq!(pre_tokenize("don't worry, be happy!"), ["don", "'", "t", "worry", ",", "be", "happy", "!"]);
    }

    #[test]
    fn reads_safetensors() {
        let header = br#"{"embeddings":{"dtype":"F32","shape":[2,2],"data_offsets":[0,16]}}"#;
        let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(header);
        for x in [1.0f32, 0.0, 0.0, 1.0] {
            bytes.extend_from_slice(&x.to_le_bytes());
        }
        let (t, rows, dim) = read_safetensors(&bytes).unwrap();
        assert_eq!((rows, dim, t), (2, 2, vec![1.0, 0.0, 0.0, 1.0]));
    }

    /// Paraphrases → the verse meant. `MEANING_MODELS=<dir containing potion-base-8M> cargo test meaning_quality -- --ignored --nocapture`
    #[test]
    #[ignore = "needs the downloaded model (MEANING_MODELS)"]
    fn meaning_quality() {
        let models = PathBuf::from(std::env::var("MEANING_MODELS").expect("MEANING_MODELS"));
        let t = std::time::Instant::now();
        let model = Model::load(&models).unwrap();
        let db = BibleDb::open(&Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/bible.db")).unwrap();
        let index = VerseIndex::build(&model, &db).unwrap();
        println!("loaded + indexed {} verses in {:?}", index.len(), t.elapsed());
        let cases: &[(&str, &str)] = &[
            ("don't worry about tomorrow, each day has enough trouble of its own", "Matthew 6:34"),
            ("God will supply everything you need", "Philippians 4:19"),
            ("I can do anything through Christ who gives me strength", "Philippians 4:13"),
            ("the Lord is my shepherd and I will never lack anything", "Psalms 23:1"),
            ("all things work together for the good of those who love God", "Romans 8:28"),
            ("God loved the world so much he gave his only son", "John 3:16"),
            ("trust in the Lord with all your heart and don't rely on your own understanding", "Proverbs 3:5"),
            ("the wages of sin is death but God's gift is eternal life", "Romans 6:23"),
            ("faith is being sure of what we hope for", "Hebrews 11:1"),
            ("be still and know that I am God", "Psalms 46:10"),
            ("those who wait on the Lord will renew their strength, they will soar like eagles", "Isaiah 40:31"),
            ("love is patient and kind, it does not envy or boast", "1 Corinthians 13:4"),
            ("there is no condemnation for those who are in Christ Jesus", "Romans 8:1"),
            ("cast all your anxiety on him because he cares for you", "1 Peter 5:7"),
            ("God has not given us a spirit of fear but of power and love", "2 Timothy 1:7"),
            ("I know the plans I have for you, plans to prosper you and not to harm you", "Jeremiah 29:11"),
            ("we are saved by grace through faith, not by our own works", "Ephesians 2:8"),
            ("if we confess our sins he is faithful to forgive us", "1 John 1:9"),
            ("your word is a lamp for my feet and a light for my path", "Psalms 119:105"),
            ("come to me all you who are weary and I will give you rest", "Matthew 11:28"),
        ];
        let mut ranks = Vec::new();
        for (q, want) in cases {
            let t = std::time::Instant::now();
            let hits = index.search(&model, q, 10);
            let el = t.elapsed();
            let rank = hits.iter().position(|h| h.reference.label().starts_with(want));
            println!("{:>4} {:.3} {:>6.1?}  {want:<22} top: {} ({:.3})  — {q}", rank.map_or("-".into(), |r| (r + 1).to_string()), rank.map_or(0.0, |r| hits[r].score), el, hits[0].reference.label(), hits[0].score);
            ranks.push(rank);
        }
        // Ordinary sermon sentences that aren't quoting anything: what do they score?
        for q in ["Good morning church, you may be seated", "This week choose one person who is hard to love and show them kindness", "Turn to your neighbour and say neighbour you look blessed", "When your colleague takes the credit for your work"] {
            let h = &index.search(&model, q, 1)[0];
            println!("noise {:.3} {} — {q}", h.score, h.reference.label());
        }
        let top1 = ranks.iter().filter(|r| **r == Some(0)).count();
        let top5 = ranks.iter().filter(|r| r.is_some_and(|r| r < 5)).count();
        println!("top-1 {top1}/{}, top-5 {top5}/{}", cases.len(), cases.len());
    }

    #[test]
    #[ignore = "downloads 30 MB from Hugging Face"]
    fn real_download() {
        let dir = std::env::temp_dir().join(format!("hk-meaning-{}", std::process::id()));
        download(&dir, |_, _| {}).unwrap();
        assert!(installed(&dir));
        download(&dir, |_, _| panic!("already there: nothing to fetch")).unwrap();
        remove(&dir).unwrap();
        assert!(!installed(&dir));
        let _ = fs::remove_dir_all(&dir);
    }
}
