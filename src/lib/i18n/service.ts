// The main service view: Preview and Live, the detection queue, transcript, finder and passage
// picker, the sidebar's plan, and the update banner.

export const fr: Record<string, string> = {
  // Detections
  Announced: "Annoncé",
  "Follow-up": "Suite",
  Quoted: "Cité",
  Song: "Chant",
  "Same meaning": "Dit autrement",
  "Voice command": "Commande vocale",
  Detections: "Détections",
  "Each new detection goes straight into Preview": "Chaque nouvelle détection va directement dans l’aperçu",
  "Preview follows latest": "L’aperçu suit la dernière",
  "References and songs appear here as they’re spoken or sung.":
    "Les références et les chants apparaissent ici dès qu’ils sont dits ou chantés.",
  "go live": "passer en direct",
  verse: "verset",
  black: "écran noir",
  clear: "effacer",
  "[♪ A song is being sung] that isn’t in your song library. Add its lyrics under Songs and Hearken will find and follow it next time.":
    "[♪ On chante un chant] absent de votre bibliothèque de chants. Ajoutez ses paroles dans Chants et Hearken le trouvera et le suivra la prochaine fois.",
  "{pct}% sure": "Sûr à {pct} %",
  "Possible quote": "Citation possible",
  "in plan": "dans le plan",
  shown: "affiché",
  "shown automatically": "affiché automatiquement",
  "ON SCREEN": "EN DIRECT",
  Live: "En direct",

  // Preview and Live
  "BLACKED OUT": "ÉCRAN NOIR",
  CLEAR: "ÉCRAN VIDE",
  Preview: "Aperçu",
  "Nothing selected": "Aucune sélection",
  "Put Preview on screen (Enter)": "Mettre l’aperçu à l’écran (Entrée)",
  "Go live": "Passer en direct",
  "⏎ Enter": "⏎ Entrée",
  "Previous verse (←)": "Verset précédent (←)",
  "Next verse (→)": "Verset suivant (→)",
  Verse: "Verset",
  "Black out the screen (B)": "Passer l’écran au noir (B)",
  Unblack: "Rallumer",
  Black: "Noir",
  "Remove the verse from the screen (Esc)": "Retirer le verset de l’écran (Échap)",
  Clear: "Effacer",

  // Search
  "Search scripture": "Rechercher un passage",
  "Press Enter again to go live": "Appuyez encore sur Entrée pour passer en direct",
  "Search: “jn 3 16”, “love is patient” or “don't worry about tomorrow”":
    "Rechercher : « jn 3 16 », « l’amour est patient » ou « ne vous inquiétez pas du lendemain »",
  "No matches": "Aucun résultat",
  "Verses that say the same thing in other words": "Des versets qui disent la même chose autrement",
  "By meaning": "Par le sens",

  // Transcript
  "Waiting for speech…": "En attente de parole…",
  "Checked by the accuracy pass": "Vérifié par la passe de précision",
  "Quick transcription": "Transcription rapide",
  "confidence {n}%": "confiance {n} %",
  "Heard sung, not spoken: not treated as a quote or a command, and left out of sermon notes":
    "Chanté, pas parlé : ni traité comme une citation ou une commande, ni repris dans les notes de la prédication",
  "♪ sung": "♪ chanté",
  checked: "vérifié",
  "Voice command: now showing {now}": "Commande vocale : affiche maintenant {now}",
  "Voice command: already at the end of the chapter": "Commande vocale : déjà à la fin du chapitre",
  "Checked lines marked ✓": "Lignes vérifiées marquées ✓",
  "The room is singing: sung Bible phrases aren't suggested as quotes, and voice commands are ignored":
    "L’assemblée chante : les phrases bibliques chantées ne sont pas proposées comme citations, et les commandes vocales sont ignorées",
  Singing: "Chant en cours",

  // Voice commands (labels sent by the listening engine)
  "Next verse": "Verset suivant",
  "Previous verse": "Verset précédent",
  "First verse": "Premier verset",
  "Last verse of the chapter": "Dernier verset du chapitre",
  "Verse {n}": "Verset {n}",
  "Verses {s}–{e}": "Versets {s}–{e}",

  // Sidebar and plan
  Plan: "Plan",
  Songs: "Chants",
  Archive: "Archives",
  "Paste the pastor's notes or verse list. Every reference is lined up below, and gets a confidence boost when it's spoken.":
    "Collez les notes du pasteur ou une liste de versets. Chaque référence est préparée ci-dessous, et sa confiance augmente quand elle est prononcée.",
  "Intro: John 3:16\n1. Rom 8:28-30\n2. Psalm 23\nClose: Philippians 4:13":
    "Intro : Jean 3:16\n1. Rom 8:28-30\n2. Psaume 23\nConclusion : Philippiens 4:13",
  "Line up references": "Préparer les références",
  "1 found": "1 trouvée",
  "{n} found": "{n} trouvées",
  "Clear plan": "Effacer le plan",
  "No references yet.": "Aucune référence pour l’instant.",

  // Passage picker
  "{book} has 1 chapter": "{book} compte 1 chapitre",
  "{book} has {n} chapters": "{book} compte {n} chapitres",
  "{book} {chapter} has {n} verses": "{book} {chapter} compte {n} versets",
  "The last verse comes before the first": "Le dernier verset vient avant le premier",
  "Ch.": "Chap.",
  Chapter: "Chapitre",
  "Pick a book first": "Choisissez d’abord un livre",
  "Whole chapter if empty": "Tout le chapitre si vide",
  to: "à",
  "Last verse (optional)": "Dernier verset (facultatif)",
  "Shows {n} verses at a time; use Verse ▶ for the rest.": "Affiche {n} versets à la fois ; utilisez Verset ▶ pour la suite.",
  Book: "Livre",
  "No book matches “{query}”": "Aucun livre ne correspond à « {query} »",
  "{n} ch.": "{n} chap.",

  // Updates
  "Version {version} is available.": "La version {version} est disponible.",
  "Stop listening before updating": "Arrêtez l’écoute avant de mettre à jour",
  "Downloading…": "Téléchargement…",
  "Installing {n}%": "Installation {n} %",
  "Install & restart": "Installer et redémarrer",
  Later: "Plus tard",
};

export const es: Record<string, string> = {
  // Detections
  Announced: "Anunciado",
  "Follow-up": "Continuación",
  Quoted: "Citado",
  Song: "Canto",
  "Same meaning": "Dicho con otras palabras",
  "Voice command": "Comando de voz",
  Detections: "Detecciones",
  "Each new detection goes straight into Preview": "Cada nueva detección pasa directamente a la vista previa",
  "Preview follows latest": "La vista previa sigue la última",
  "References and songs appear here as they’re spoken or sung.":
    "Las referencias y los cantos aparecen aquí en cuanto se dicen o se cantan.",
  "go live": "poner en vivo",
  verse: "versículo",
  black: "pantalla negra",
  clear: "borrar",
  "[♪ A song is being sung] that isn’t in your song library. Add its lyrics under Songs and Hearken will find and follow it next time.":
    "[♪ Se está cantando un canto] que no está en tu biblioteca de cantos. Agrega su letra en Cantos y Hearken lo encontrará y lo seguirá la próxima vez.",
  "{pct}% sure": "{pct}% de certeza",
  "Possible quote": "Posible cita",
  "in plan": "en el plan",
  shown: "mostrado",
  "shown automatically": "mostrado automáticamente",
  "ON SCREEN": "EN VIVO",
  Live: "En vivo",

  // Preview and Live
  "BLACKED OUT": "PANTALLA NEGRA",
  CLEAR: "PANTALLA VACÍA",
  Preview: "Vista previa",
  "Nothing selected": "Nada seleccionado",
  "Put Preview on screen (Enter)": "Poner la vista previa en pantalla (Enter)",
  "Go live": "Poner en vivo",
  "Previous verse (←)": "Versículo anterior (←)",
  "Next verse (→)": "Versículo siguiente (→)",
  Verse: "Versículo",
  "Black out the screen (B)": "Poner la pantalla en negro (B)",
  Unblack: "Quitar negro",
  Black: "Negro",
  "Remove the verse from the screen (Esc)": "Quitar el versículo de la pantalla (Esc)",
  Clear: "Borrar",

  // Search
  "Search scripture": "Buscar un pasaje",
  "Press Enter again to go live": "Pulsa Enter otra vez para poner en vivo",
  "Search: “jn 3 16”, “love is patient” or “don't worry about tomorrow”":
    "Buscar: «jn 3 16», «el amor es paciente» o «no te preocupes por el mañana»",
  "No matches": "Sin resultados",
  "Verses that say the same thing in other words": "Versículos que dicen lo mismo con otras palabras",
  "By meaning": "Por significado",

  // Transcript
  "Waiting for speech…": "Esperando a que alguien hable…",
  "Checked by the accuracy pass": "Revisado por la pasada de precisión",
  "Quick transcription": "Transcripción rápida",
  "confidence {n}%": "confianza {n}%",
  "Heard sung, not spoken: not treated as a quote or a command, and left out of sermon notes":
    "Cantado, no hablado: no se trata como cita ni como comando, y queda fuera de las notas del sermón",
  "♪ sung": "♪ cantado",
  checked: "revisado",
  "Voice command: now showing {now}": "Comando de voz: ahora se muestra {now}",
  "Voice command: already at the end of the chapter": "Comando de voz: ya está al final del capítulo",
  "Checked lines marked ✓": "Líneas revisadas marcadas con ✓",
  "The room is singing: sung Bible phrases aren't suggested as quotes, and voice commands are ignored":
    "La congregación está cantando: las frases bíblicas cantadas no se sugieren como citas y los comandos de voz se ignoran",
  Singing: "Cantando",

  // Voice commands (labels sent by the listening engine)
  "Next verse": "Versículo siguiente",
  "Previous verse": "Versículo anterior",
  "First verse": "Primer versículo",
  "Last verse of the chapter": "Último versículo del capítulo",
  "Verse {n}": "Versículo {n}",
  "Verses {s}–{e}": "Versículos {s}–{e}",

  // Sidebar and plan
  Plan: "Plan",
  Songs: "Cantos",
  Archive: "Archivo",
  "Paste the pastor's notes or verse list. Every reference is lined up below, and gets a confidence boost when it's spoken.":
    "Pega las notas del pastor o una lista de versículos. Cada referencia queda preparada abajo y gana confianza cuando se menciona.",
  "Intro: John 3:16\n1. Rom 8:28-30\n2. Psalm 23\nClose: Philippians 4:13":
    "Introducción: Juan 3:16\n1. Rom 8:28-30\n2. Salmo 23\nCierre: Filipenses 4:13",
  "Line up references": "Preparar referencias",
  "1 found": "1 encontrada",
  "{n} found": "{n} encontradas",
  "Clear plan": "Borrar el plan",
  "No references yet.": "Aún no hay referencias.",

  // Passage picker
  "{book} has 1 chapter": "{book} tiene 1 capítulo",
  "{book} has {n} chapters": "{book} tiene {n} capítulos",
  "{book} {chapter} has {n} verses": "{book} {chapter} tiene {n} versículos",
  "The last verse comes before the first": "El último versículo va antes del primero",
  "Ch.": "Cap.",
  Chapter: "Capítulo",
  "Pick a book first": "Elige primero un libro",
  "Whole chapter if empty": "Todo el capítulo si está vacío",
  to: "a",
  "Last verse (optional)": "Último versículo (opcional)",
  "Shows {n} verses at a time; use Verse ▶ for the rest.": "Muestra {n} versículos a la vez; usa Versículo ▶ para el resto.",
  Book: "Libro",
  "No book matches “{query}”": "Ningún libro coincide con «{query}»",
  "{n} ch.": "{n} cap.",

  // Updates
  "Version {version} is available.": "La versión {version} está disponible.",
  "Stop listening before updating": "Detén la escucha antes de actualizar",
  "Downloading…": "Descargando…",
  "Installing {n}%": "Instalando {n}%",
  "Install & restart": "Instalar y reiniciar",
  Later: "Más tarde",
};
