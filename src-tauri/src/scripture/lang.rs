//! Preaching languages. Everything that listens takes its words from here: how references are
//! said ("chapitre", "versículo"), spoken numbers, the books' names, the little words that carry
//! no meaning in a quote, the speech model's prompt, and the Bible shown by default. Adding a
//! language is mostly adding its tables.
//!
//! Words are compared without accents (`fold`): speech recognition writes "Genese" as often as
//! "Genèse", and tone-marked languages often lose their marks. All the tables are folded.

use rust_stemmers::Algorithm;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Lang {
    En,
    Fr,
    Es,
}

/// Lowercase text without accents or tone marks ("Ésaïe" → "esaie", "mẹ́ẹ̀ẹ́dógún" → "meeedogun").
pub fn fold(text: &str) -> String {
    let lower = text.to_lowercase();
    if lower.is_ascii() {
        return lower;
    }
    lower.nfkd().filter(|c| !is_combining_mark(*c)).collect()
}

/// How a language says its numbers. Numbers between 0 and 999 are all a reference needs.
pub(super) struct Numbers {
    /// 0 to 19 (French stops at 16: "dix-sept" is ten and seven).
    pub units: &'static [(&'static str, u32)],
    pub tens: &'static [(&'static str, u32)],
    pub hundreds: &'static [&'static str],
    /// Whole numbers said as one word ("veintiocho").
    pub wholes: &'static [(&'static str, u32)],
    /// Also everyday words ("un jour", "una vez"): a number only beside another number or "chapter"/"verse".
    pub articles: &'static [&'static str],
    /// Also English words ("once", "seize"): treated like articles when English is listened for too.
    pub english_words: &'static [&'static str],
    /// Joins tens and units ("vingt et un", "treinta y uno"), or a hundred and the rest ("a hundred and ten").
    pub connector: Connector,
    /// French counting in twenties: "soixante-dix" (60 + 10), "quatre-vingt" (4 × 20), "dix-sept" (10 + 7).
    pub vigesimal: bool,
}

#[derive(Clone, Copy)]
pub(super) enum Connector {
    AfterHundred(&'static str),
    AfterTen(&'static str),
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::En, Lang::Fr, Lang::Es];

    /// ISO 639-1 code, as Whisper and the settings name it.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::Es => "es",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.code().eq_ignore_ascii_case(code.trim()))
    }

    /// Codes from the settings, in order, without repeats or unknown ones; English if none.
    pub fn list(codes: &[String]) -> Vec<Lang> {
        let mut out: Vec<Lang> = Vec::new();
        for l in codes.iter().filter_map(|c| Self::from_code(c)) {
            if !out.contains(&l) {
                out.push(l);
            }
        }
        if out.is_empty() {
            out.push(Lang::En);
        }
        out
    }

    /// The language's English name, as the Bible library labels translations.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Fr => "French",
            Lang::Es => "Spanish",
        }
    }

    pub fn native(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Fr => "Français",
            Lang::Es => "Español",
        }
    }

    /// The translation shown by default when this language is preached.
    pub fn bible(self) -> &'static str {
        match self {
            Lang::En => "KJV",
            Lang::Fr => "LSG",
            Lang::Es => "RV1909",
        }
    }

    /// The language of a translation (bundled ones are English).
    pub fn of_translation(code: &str) -> Lang {
        super::library::entry(code).and_then(|e| Self::ALL.into_iter().find(|l| l.name() == e.language)).unwrap_or(Lang::En)
    }

    pub fn stemmer(self) -> Algorithm {
        match self {
            Lang::En => Algorithm::English,
            Lang::Fr => Algorithm::French,
            Lang::Es => Algorithm::Spanish,
        }
    }

    /// The 66 books' names in this language, in canon order (for prompts).
    pub fn book_names(self) -> Vec<String> {
        match self {
            Lang::En => super::books::BOOKS.iter().map(|b| b.display.to_owned()).collect(),
            Lang::Fr => super::library::book_names("fraLSG"),
            Lang::Es => super::library::book_names("spaRV1909"),
        }
    }

    /// Shown to the speech model before each decode: biases it toward Bible vocabulary and
    /// toward writing references as "Book C:V" digits, which is far easier to parse.
    pub fn prompt(self) -> &'static str {
        match self {
            Lang::En => "Sermon transcript with scripture references such as John 3:16, Romans 8:28, \
1 Corinthians 13:4-7, Philippians 4:13, Psalm 23:1, Habakkuk 2:3, Ecclesiastes 3:1, Deuteronomy 31:6, \
Exodus 14:14, Isaiah 40:31, Ezekiel 37:4, Galatians 5:22, Leviticus 19:18, Revelation 21:4.",
            Lang::Fr => "Prédication avec des références bibliques comme Jean 3:16, Romains 8:28, \
1 Corinthiens 13:4-7, Philippiens 4:13, Psaume 23:1, Habacuc 2:3, Ecclésiaste 3:1, Deutéronome 31:6, \
Exode 14:14, Ésaïe 40:31, Ézéchiel 37:4, Galates 5:22, Lévitique 19:18, Apocalypse 21:4.",
            Lang::Es => "Predicación con citas bíblicas como Juan 3:16, Romanos 8:28, 1 Corintios 13:4-7, \
Filipenses 4:13, Salmo 23:1, Habacuc 2:3, Eclesiastés 3:1, Deuteronomio 31:6, Éxodo 14:14, Isaías 40:31, \
Ezequiel 37:4, Gálatas 5:22, Levítico 19:18, Apocalipsis 21:4.",
        }
    }

    /// Opening of the accuracy pass's prompt, before the list of books.
    pub fn books_heading(self) -> &'static str {
        match self {
            Lang::En => "Sermon transcript. Books of the Bible:",
            Lang::Fr => "Prédication. Livres de la Bible :",
            Lang::Es => "Predicación. Libros de la Biblia:",
        }
    }

    pub fn readings_heading(self) -> &'static str {
        match self {
            Lang::En => "Today's readings:",
            Lang::Fr => "Lectures du jour :",
            Lang::Es => "Lecturas de hoy:",
        }
    }

    /// What the speech model writes on silence, music or applause (lowercase).
    pub fn hallucinations(self) -> &'static [&'static str] {
        match self {
            Lang::En => &["thank you.", "thank you", "thanks for watching!", "thank you for watching.", "you", "bye.", "."],
            Lang::Fr => &[
                "merci.", "merci", "merci d'avoir regardé.", "sous-titrage société radio-canada",
                "sous-titres réalisés par la communauté d'amara.org", "sous-titrage st' 501", ".",
            ],
            Lang::Es => &[
                "gracias.", "gracias", "¡gracias!", "gracias por ver el video.", "¡suscríbete!",
                "subtítulos realizados por la comunidad de amara.org", ".",
            ],
        }
    }

    /// Little words that carry no meaning of their own in a quote ("the", "le", "el").
    pub fn stopwords(self) -> &'static [&'static str] {
        match self {
            Lang::En => &[
                "the", "and", "of", "to", "a", "in", "that", "is", "for", "it", "as", "was", "with", "be", "his", "he", "i",
                "you", "on", "by", "not", "but", "are", "this", "they", "have", "from", "or", "at", "all", "we", "my", "me",
                "your", "so", "will", "unto", "thee", "thou", "thy", "shall", "him", "them", "which", "there", "what", "when",
                "who", "were", "their", "an", "if", "our", "us", "do", "no", "hath", "ye", "then", "said", "into", "up", "out",
                "now", "also", "been", "had", "has", "her", "she", "its", "these", "those", "would", "could", "let", "like",
                "just", "very", "okay", "amen", "can", "say", "says", "saying", "know", "going", "get", "got", "one",
            ],
            Lang::Fr => &[
                "le", "la", "les", "l", "de", "des", "du", "d", "un", "une", "et", "a", "au", "aux", "en", "que", "qu", "qui",
                "est", "sont", "etait", "ete", "etre", "il", "elle", "ils", "elles", "je", "j", "tu", "nous", "vous", "on",
                "ne", "n", "pas", "plus", "se", "s", "sa", "son", "ses", "ce", "c", "cet", "cette", "ces", "mon", "ma", "mes",
                "ton", "ta", "tes", "notre", "nos", "votre", "vos", "leur", "leurs", "lui", "me", "m", "te", "t", "y", "dans",
                "par", "pour", "sur", "avec", "sans", "mais", "ou", "donc", "car", "ni", "si", "comme", "tout", "tous",
                "toute", "toutes", "aussi", "bien", "tres", "alors", "cela", "ca", "ai", "as", "avons", "avez", "ont",
                "avait", "fait", "dit", "oui", "non", "voici", "voila", "amen",
            ],
            Lang::Es => &[
                "el", "la", "los", "las", "lo", "un", "una", "unos", "unas", "de", "del", "al", "a", "y", "e", "o", "u", "en",
                "que", "quien", "es", "son", "fue", "era", "ser", "esta", "este", "esto", "estos", "estas", "ese", "esa",
                "eso", "se", "su", "sus", "mi", "mis", "tu", "tus", "nuestro", "nuestra", "nuestros", "le", "les", "me",
                "te", "nos", "os", "yo", "ella", "ellos", "ellas", "nosotros", "usted", "ustedes", "por", "para", "con",
                "sin", "sobre", "pero", "mas", "como", "si", "no", "ya", "muy", "tambien", "todo", "todos", "toda",
                "todas", "hay", "ha", "han", "he", "dijo", "dice", "asi", "porque", "cuando", "donde", "amen",
            ],
        }
    }

    pub(super) fn numbers(self) -> &'static Numbers {
        match self {
            Lang::En => &EN_NUMBERS,
            Lang::Fr => &FR_NUMBERS,
            Lang::Es => &ES_NUMBERS,
        }
    }

    /// The language's words for "chapter" and "verse", as the English ones the parser reads.
    pub(super) fn keywords(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Lang::En => &[],
            Lang::Fr => &[("chapitre", "chapter"), ("chapitres", "chapters"), ("chap", "chapter"), ("verset", "verse"), ("versets", "verses")],
            Lang::Es => &[
                ("capitulo", "chapter"), ("capitulos", "chapters"), ("cap", "chapter"), ("versiculo", "verse"),
                ("versiculos", "verses"), ("verso", "verse"), ("versos", "verses"),
            ],
        }
    }

    /// Ordinals before numbered books ("Première épître de Jean", "Segunda de Timoteo") as digits.
    pub(super) fn ordinals(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Lang::En => &[],
            Lang::Fr => &[
                ("premier", "1"), ("premiere", "1"), ("1er", "1"), ("1re", "1"), ("1ere", "1"),
                ("deuxieme", "2"), ("second", "2"), ("seconde", "2"), ("2e", "2"), ("2eme", "2"), ("2nd", "2"), ("2nde", "2"), ("ii", "2"),
                ("troisieme", "3"), ("3e", "3"), ("3eme", "3"), ("iii", "3"),
            ],
            Lang::Es => &[
                ("primera", "1"), ("primero", "1"), ("primer", "1"), ("1a", "1"), ("1o", "1"), ("1ra", "1"), ("1ro", "1"), ("1er", "1"),
                ("segunda", "2"), ("segundo", "2"), ("2a", "2"), ("2o", "2"), ("2da", "2"), ("2do", "2"), ("ii", "2"),
                ("tercera", "3"), ("tercero", "3"), ("tercer", "3"), ("3a", "3"), ("3o", "3"), ("3ra", "3"), ("3ro", "3"), ("3er", "3"), ("iii", "3"),
            ],
        }
    }

    /// Words for "to" in a range ("versets 4 à 7", "del 4 al 7"), when between numbers.
    pub(super) fn range_words(self) -> &'static [&'static str] {
        match self {
            Lang::En => &[],
            Lang::Fr => &["a", "au", "aux", "jusqua", "jusquau", "jusquaux"],
            Lang::Es => &["a", "al", "hasta"],
        }
    }

    /// Words for "and" ("versets 4 et 5").
    pub(super) fn and_words(self) -> &'static [&'static str] {
        match self {
            Lang::En => &[],
            Lang::Fr => &["et"],
            Lang::Es => &["y"],
        }
    }

    /// Small words before "chapter"/"verse" that the parser doesn't need ("du verset", "del versículo").
    pub(super) fn determiners(self) -> &'static [&'static str] {
        match self {
            Lang::En => &[],
            Lang::Fr => &["du", "de", "des", "le", "les", "la", "au", "aux", "a"],
            Lang::Es => &["del", "de", "el", "los", "la", "las", "desde", "al", "a"],
        }
    }

    /// The books' names: (book ids, spellings, weak). Numbered books list the base name for all
    /// their ids; the ordinal is added. Weak names are also everyday words or common first
    /// names ("Marc", "Pierre", "Santiago"), so they need stronger evidence. English names are
    /// in `books::BOOKS`.
    pub(super) fn books(self) -> &'static [(&'static [u8], &'static [&'static str], bool)] {
        match self {
            Lang::En => &[],
            Lang::Fr => FR_BOOKS,
            Lang::Es => ES_BOOKS,
        }
    }

    /// Words between an ordinal and a numbered book's name ("1 épître aux Corinthiens",
    /// "1 de Corintios").
    pub(super) fn ordinal_links(self) -> &'static [&'static str] {
        match self {
            Lang::En => &[""],
            Lang::Fr => &["", "aux ", "a ", "de ", "epitre aux ", "epitre a ", "epitre de ", "lettre aux ", "lettre a ", "lettre de ", "livre de ", "livre des "],
            Lang::Es => &[
                "", "de ", "a ", "a los ", "carta a ", "carta a los ", "carta de ", "epistola a ", "epistola a los ", "epistola de ",
                "libro de ", "libro de los ", "libro de las ",
            ],
        }
    }
}

/// The language most of these words are in (by their little words), English when unsure.
pub fn guess(text: &str) -> Lang {
    let words: Vec<String> = fold(text).split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect();
    let score = |l: Lang| words.iter().filter(|w| l.stopwords().contains(&w.as_str())).count();
    Lang::ALL.into_iter().max_by_key(|&l| (score(l), l == Lang::En)).unwrap_or(Lang::En)
}

static EN_NUMBERS: Numbers = Numbers {
    units: &[
        ("zero", 0), ("one", 1), ("two", 2), ("three", 3), ("four", 4), ("five", 5), ("six", 6), ("seven", 7), ("eight", 8),
        ("nine", 9), ("ten", 10), ("eleven", 11), ("twelve", 12), ("thirteen", 13), ("fourteen", 14), ("fifteen", 15),
        ("sixteen", 16), ("seventeen", 17), ("eighteen", 18), ("nineteen", 19),
    ],
    tens: &[("twenty", 20), ("thirty", 30), ("forty", 40), ("fourty", 40), ("fifty", 50), ("sixty", 60), ("seventy", 70), ("eighty", 80), ("ninety", 90)],
    hundreds: &["hundred"],
    wholes: &[],
    articles: &[],
    english_words: &[],
    connector: Connector::AfterHundred("and"),
    vigesimal: false,
};

static FR_NUMBERS: Numbers = Numbers {
    units: &[
        ("zero", 0), ("un", 1), ("une", 1), ("deux", 2), ("trois", 3), ("quatre", 4), ("cinq", 5), ("six", 6), ("sept", 7),
        ("huit", 8), ("neuf", 9), ("dix", 10), ("onze", 11), ("douze", 12), ("treize", 13), ("quatorze", 14), ("quinze", 15),
        ("seize", 16),
    ],
    tens: &[
        ("vingt", 20), ("vingts", 20), ("trente", 30), ("quarante", 40), ("cinquante", 50), ("soixante", 60), ("septante", 70),
        ("huitante", 80), ("octante", 80), ("nonante", 90),
    ],
    hundreds: &["cent", "cents"],
    wholes: &[],
    articles: &["un", "une"],
    english_words: &["sept", "cent", "seize"],
    connector: Connector::AfterTen("et"),
    vigesimal: true,
};

static ES_NUMBERS: Numbers = Numbers {
    units: &[
        ("cero", 0), ("uno", 1), ("un", 1), ("una", 1), ("dos", 2), ("tres", 3), ("cuatro", 4), ("cinco", 5), ("seis", 6),
        ("siete", 7), ("ocho", 8), ("nueve", 9), ("diez", 10), ("once", 11), ("doce", 12), ("trece", 13), ("catorce", 14),
        ("quince", 15), ("dieciseis", 16), ("diecisiete", 17), ("dieciocho", 18), ("diecinueve", 19),
    ],
    tens: &[("veinte", 20), ("treinta", 30), ("cuarenta", 40), ("cincuenta", 50), ("sesenta", 60), ("setenta", 70), ("ochenta", 80), ("noventa", 90)],
    hundreds: &["cien", "ciento"],
    wholes: &[
        ("veintiuno", 21), ("veintiun", 21), ("veintidos", 22), ("veintitres", 23), ("veinticuatro", 24), ("veinticinco", 25),
        ("veintiseis", 26), ("veintisiete", 27), ("veintiocho", 28), ("veintinueve", 29),
    ],
    articles: &["un", "una", "uno"],
    english_words: &["once", "dos"],
    connector: Connector::AfterTen("y"),
    vigesimal: false,
};

/// Louis Segond names, with the Catholic spellings (Isaïe, Qohélet) and common abbreviations.
static FR_BOOKS: &[(&[u8], &[&str], bool)] = &[
    (&[1], &["genese", "gn"], false),
    (&[2], &["exode"], false),
    (&[3], &["levitique", "lv"], false),
    (&[4], &["nombres"], true),
    (&[5], &["deuteronome", "dt"], false),
    (&[6], &["josue"], false),
    (&[7], &["juges"], true),
    (&[8], &["ruth"], true),
    (&[9, 10], &["samuel"], false),
    (&[11, 12], &["rois"], true),
    (&[13, 14], &["chroniques", "chronique"], false),
    (&[15], &["esdras"], false),
    (&[16], &["nehemie"], false),
    (&[17], &["esther"], false),
    (&[18], &["job"], true),
    (&[19], &["psaumes", "psaume"], false),
    (&[20], &["proverbes"], false),
    (&[20], &["proverbe"], true),
    (&[21], &["ecclesiaste", "qohelet"], false),
    (&[22], &["cantique des cantiques", "cantiques des cantiques"], false),
    (&[22], &["cantique"], true), // also a hymn: "cantique 4"
    (&[23], &["esaie", "isaie"], false),
    (&[24], &["jeremie"], false),
    (&[25], &["lamentations"], false),
    (&[26], &["ezechiel"], false),
    (&[27], &["daniel"], true),
    (&[28], &["osee"], false),
    (&[29], &["joel"], true),
    (&[30], &["amos"], true),
    (&[31], &["abdias"], false),
    (&[32], &["jonas"], true),
    (&[33], &["michee"], false),
    (&[34], &["nahum"], false),
    (&[35], &["habacuc"], false),
    (&[36], &["sophonie"], false),
    (&[37], &["aggee"], false),
    (&[38], &["zacharie"], false),
    (&[39], &["malachie"], false),
    (&[40], &["matthieu", "mathieu"], false),
    (&[41], &["marc"], true),
    (&[42], &["luc"], true),
    (&[43, 62, 63, 64], &["jean"], false),
    (&[44], &["actes des apotres"], false),
    (&[44], &["actes"], true),
    (&[45], &["romains"], false),
    (&[46, 47], &["corinthiens", "corinthien"], false),
    (&[48], &["galates"], false),
    (&[49], &["ephesiens"], false),
    (&[50], &["philippiens", "philipiens"], false),
    (&[51], &["colossiens"], false),
    (&[52, 53], &["thessaloniciens"], false),
    (&[54, 55], &["timothee"], false),
    (&[56], &["tite"], true),
    (&[57], &["philemon"], false),
    (&[58], &["hebreux"], false),
    (&[59], &["jacques"], true),
    (&[60, 61], &["pierre"], true),
    (&[65], &["jude"], true),
    (&[66], &["apocalypse"], false),
];

/// Reina-Valera names, with "San Juan" and the like, and the other usual spellings.
static ES_BOOKS: &[(&[u8], &[&str], bool)] = &[
    (&[1], &["genesis"], false),
    (&[2], &["exodo"], false),
    (&[3], &["levitico"], false),
    (&[4], &["numeros"], true),
    (&[5], &["deuteronomio"], false),
    (&[6], &["josue"], false),
    (&[7], &["jueces"], true),
    (&[8], &["rut"], true),
    (&[9, 10], &["samuel"], false),
    (&[11, 12], &["reyes"], true),
    (&[13, 14], &["cronicas"], false),
    (&[15], &["esdras"], false),
    (&[16], &["nehemias"], false),
    (&[17], &["ester"], true),
    (&[18], &["job"], true),
    (&[19], &["salmos", "salmo"], false),
    (&[20], &["proverbios"], false),
    (&[21], &["eclesiastes"], false),
    (&[22], &["cantar de los cantares", "cantares", "cantico de los canticos"], false),
    (&[23], &["isaias"], false),
    (&[24], &["jeremias"], false),
    (&[25], &["lamentaciones"], false),
    (&[26], &["ezequiel"], false),
    (&[27], &["daniel"], true),
    (&[28], &["oseas"], false),
    (&[29], &["joel"], true),
    (&[30], &["amos"], true),
    (&[31], &["abdias"], false),
    (&[32], &["jonas"], true),
    (&[33], &["miqueas"], false),
    (&[34], &["nahum"], false),
    (&[35], &["habacuc"], false),
    (&[36], &["sofonias"], false),
    (&[37], &["hageo"], false),
    (&[38], &["zacarias"], false),
    (&[39], &["malaquias"], false),
    (&[40], &["mateo", "san mateo"], false),
    (&[41], &["san marcos"], false),
    (&[41], &["marcos"], true),
    (&[42], &["san lucas"], false),
    (&[42], &["lucas"], true),
    (&[43], &["san juan"], false),
    (&[43, 62, 63, 64], &["juan"], false),
    (&[44], &["hechos de los apostoles"], false),
    (&[44], &["hechos"], true),
    (&[45], &["romanos"], false),
    (&[46, 47], &["corintios"], false),
    (&[48], &["galatas"], false),
    (&[49], &["efesios"], false),
    (&[50], &["filipenses"], false),
    (&[51], &["colosenses"], false),
    (&[52, 53], &["tesalonicenses"], false),
    (&[54, 55], &["timoteo"], false),
    (&[56], &["tito"], true),
    (&[57], &["filemon"], false),
    (&[58], &["hebreos"], false),
    (&[59], &["santiago"], true),
    (&[60, 61], &["pedro"], true),
    (&[65], &["judas"], true),
    (&[66], &["apocalipsis"], false),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_drops_accents_and_tone_marks() {
        assert_eq!(fold("Ésaïe Genèse Éxodo Nehemías"), "esaie genese exodo nehemias");
        assert_eq!(fold("Ọlọ́run"), "olorun");
        assert_eq!(fold("1ª"), "1a");
    }

    #[test]
    fn settings_codes() {
        assert_eq!(Lang::list(&["fr".into(), "EN".into(), "fr".into(), "xx".into()]), vec![Lang::Fr, Lang::En]);
        assert_eq!(Lang::list(&[]), vec![Lang::En]);
        assert_eq!(Lang::of_translation("LSG"), Lang::Fr);
        assert_eq!(Lang::of_translation("rv1909"), Lang::Es);
        assert_eq!(Lang::of_translation("KJV"), Lang::En);
    }

    #[test]
    fn tables_are_folded_and_complete() {
        for l in Lang::ALL {
            let mut covered = [false; 66];
            for (ids, spellings, _) in l.books() {
                for s in *spellings {
                    assert_eq!(fold(s), *s, "{l:?} {s}");
                }
                for &id in *ids {
                    covered[id as usize - 1] = true;
                }
            }
            assert!(l == Lang::En || covered.iter().all(|&c| c), "{l:?} names every book");
            for w in l.stopwords() {
                assert_eq!(fold(w), *w, "{l:?} {w}");
            }
            assert_eq!(l.book_names().len(), 66, "{l:?}");
        }
    }

    #[test]
    fn guesses_the_language_of_a_sermon() {
        assert_eq!(guess("For God so loved the world that he gave his only Son"), Lang::En);
        assert_eq!(guess("Car Dieu a tant aimé le monde qu'il a donné son Fils unique"), Lang::Fr);
        assert_eq!(guess("Porque de tal manera amó Dios al mundo, que ha dado a su Hijo"), Lang::Es);
        assert_eq!(guess(""), Lang::En);
    }
}
