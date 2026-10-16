// The top bar, Settings menu, first-run screens and Language dialog, and messages from the
// listening engine shown in the dashboard's banner.

export const fr: Record<string, string> = {
  // Languages
  English: "Anglais",
  French: "Français",
  Spanish: "Espagnol",
  "Which languages are preached at your church?": "Dans quelles langues prêche-t-on dans votre église ?",
  "Choose more than one if your services mix languages. Yoruba, Hausa and more are on the way.":
    "Choisissez-en plusieurs si vos cultes mélangent les langues. Le yoruba, le haoussa et d’autres arrivent bientôt.",
  "Hearken’s menus and buttons": "Les menus et boutons de Hearken",
  "Stay in English": "Rester en anglais",
  "{language} is used for listening only.": "{language} : seulement pour l’écoute.",
  "Menus and buttons change to {language} too.": "Les menus et boutons aussi : {language}.",
  "Speech model · {model}": "Modèle vocal · {model}",
  "{language} speech model": "Modèle vocal ({language})",
  "Base (all languages)": "Base (toutes langues)",
  "Tiny (all languages)": "Tiny (toutes langues)",
  "Small (all languages)": "Small (toutes langues)",
  Ready: "Prêt",
  "Not downloaded": "Pas téléchargé",
  Language: "Langue",
  "To listen offline, Hearken needs:": "Pour écouter hors ligne, Hearken a besoin de :",
  "Listening changes the next time you start listening.": "L’écoute changera la prochaine fois que vous la démarrerez.",
  Save: "Enregistrer",
  "Download what’s missing": "Télécharger ce qui manque",
  "Downloads carry on if you close this.": "Les téléchargements continuent si vous fermez cette fenêtre.",
  "Welcome to Hearken": "Bienvenue dans Hearken",
  "Step {n} of {total}": "Étape {n} sur {total}",
  "Hearken listens to the sermon and puts each scripture on the screen as it’s read. First, the language.":
    "Hearken écoute la prédication et affiche chaque passage biblique à l’écran au moment où il est lu. D’abord, la langue.",
  "To listen offline, Hearken will download:": "Pour écouter hors ligne, Hearken va télécharger :",
  "You can change this later in Settings.": "Vous pourrez changer cela plus tard dans les Réglages.",
  "One moment…": "Un instant…",
  Continue: "Continuer",
  "Not now: I’ll sign in when I start listening": "Plus tard : je me connecterai en démarrant l’écoute",

  // Top bar
  "Stop listening to change the input": "Arrêtez l’écoute pour changer d’entrée",
  "Sound input": "Entrée audio",
  "Saved input": "Entrée enregistrée",
  "System default input": "Entrée par défaut du système",
  Translation: "Traduction",
  "Side by side": "Côte à côte",
  "Show a second translation (or language) next to the first": "Afficher une deuxième traduction (ou langue) à côté de la première",
  "Open the projector output on the chosen display (Settings → Output display). Press Esc on it to close it.":
    "Ouvrir l’affichage du projecteur sur l’écran choisi (Réglages → Écran de sortie). Appuyez sur Échap dessus pour le fermer.",
  "Open output": "Ouvrir l’écran",
  "Stacked: one above the other. Click to put them side by side.": "Empilées : l’une au-dessus de l’autre. Cliquez pour les mettre côte à côte.",
  "Side by side. Click to stack them one above the other.": "Côte à côte. Cliquez pour les empiler l’une au-dessus de l’autre.",
  "Put translations side by side": "Mettre les traductions côte à côte",
  "Stack translations": "Empiler les traductions",
  "Stop listening": "Arrêter l’écoute",
  "Start listening": "Commencer l’écoute",
  "Starting…": "Démarrage…",
  Listening: "À l’écoute",

  // Settings menu
  Settings: "Réglages",
  "Fonts, colours, backgrounds": "Polices, couleurs, arrière-plans",
  Style: "Style",
  "Models, names to listen for, search by meaning": "Modèles, noms à reconnaître, recherche par le sens",
  "Transcription accuracy": "Précision de la transcription",
  "Yoruba, Igbo, Hausa and more": "Yoruba, igbo, haoussa et plus",
  Bibles: "Bibles",
  "Preaching languages, menus and buttons": "Langues de prédication, menus et boutons",
  "Account, free trial, licence key": "Compte, essai gratuit, clé de licence",
  "Account and licence": "Compte et licence",
  "During the service": "Pendant le culte",
  "Follow reading": "Suivre la lecture",
  "Highlight the live passage as it's read and move to the next verse automatically":
    "Surligner le passage en direct pendant sa lecture et passer automatiquement au verset suivant",
  "Follow worship songs": "Suivre les chants",
  "Recognise songs from your library as they're sung and follow them section by section":
    "Reconnaître les chants de votre bibliothèque quand ils sont chantés, et les suivre partie par partie",
  "Voice commands (“next verse”, “go back”)": "Commandes vocales (« verset suivant », « en arrière »)",
  "Move the live scripture when the preacher says “next verse”, “previous verse”, “go back” or “go to the first verse”":
    "Déplacer le passage en direct quand le prédicateur dit « verset suivant », « verset précédent », « en arrière » ou « allons au premier verset »",
  "Show sure detections automatically, at": "Afficher automatiquement les détections sûres, à partir de",
  "Automatic threshold": "Seuil automatique",
  "Output display": "Écran de sortie",
  "Automatic (second display)": "Automatique (deuxième écran)",
  primary: "principal",
  "Close the output window": "Fermer la fenêtre de sortie",
  "Speech engine": "Moteur vocal",
  "Not benchmarked yet — runs automatically on first start": "Pas encore testé — le test se fait automatiquement au premier démarrage",
  "Re-benchmark": "Refaire le test",

  // Dashboard
  Transcript: "Transcription",
  Dismiss: "Ignorer",
  "No sound is reaching Hearken Studio. If macOS blocked the microphone, allow it in System Settings → Privacy & Security → Microphone, then stop and start listening. Otherwise check the input isn't muted.":
    "Aucun son n’arrive à Hearken Studio. Si macOS a bloqué le micro, autorisez-le dans Réglages Système → Confidentialité et sécurité → Microphone, puis arrêtez et relancez l’écoute. Sinon, vérifiez que l’entrée n’est pas coupée.",
  "No sound is reaching Hearken Studio. Check the input device isn't muted and that microphone access is allowed in the system privacy settings, then stop and start listening.":
    "Aucun son n’arrive à Hearken Studio. Vérifiez que l’entrée n’est pas coupée et que l’accès au micro est autorisé dans les réglages de confidentialité du système, puis arrêtez et relancez l’écoute.",
};

export const es: Record<string, string> = {
  // Languages
  English: "Inglés",
  French: "Francés",
  Spanish: "Español",
  "Which languages are preached at your church?": "¿En qué idiomas se predica en tu iglesia?",
  "Choose more than one if your services mix languages. Yoruba, Hausa and more are on the way.":
    "Elige más de uno si tus cultos mezclan idiomas. El yoruba, el hausa y otros llegarán pronto.",
  "Hearken’s menus and buttons": "Los menús y botones de Hearken",
  "Stay in English": "Seguir en inglés",
  "{language} is used for listening only.": "{language}: solo para escuchar.",
  "Menus and buttons change to {language} too.": "Menús y botones también: {language}.",
  "Speech model · {model}": "Modelo de voz · {model}",
  "{language} speech model": "Modelo de voz ({language})",
  "Base (all languages)": "Base (todos los idiomas)",
  "Tiny (all languages)": "Tiny (todos los idiomas)",
  "Small (all languages)": "Small (todos los idiomas)",
  Ready: "Listo",
  "Not downloaded": "Sin descargar",
  Language: "Idioma",
  "To listen offline, Hearken needs:": "Para escuchar sin conexión, Hearken necesita:",
  "Listening changes the next time you start listening.": "La escucha cambiará la próxima vez que la inicies.",
  Save: "Guardar",
  "Download what’s missing": "Descargar lo que falta",
  "Downloads carry on if you close this.": "Las descargas continúan aunque cierres esto.",
  "Welcome to Hearken": "Bienvenido a Hearken",
  "Step {n} of {total}": "Paso {n} de {total}",
  "Hearken listens to the sermon and puts each scripture on the screen as it’s read. First, the language.":
    "Hearken escucha la predicación y muestra cada pasaje bíblico en la pantalla mientras se lee. Primero, el idioma.",
  "To listen offline, Hearken will download:": "Para escuchar sin conexión, Hearken descargará:",
  "You can change this later in Settings.": "Puedes cambiarlo más tarde en Ajustes.",
  "One moment…": "Un momento…",
  Continue: "Continuar",
  "Not now: I’ll sign in when I start listening": "Ahora no: iniciaré sesión cuando empiece a escuchar",

  // Top bar
  "Stop listening to change the input": "Detén la escucha para cambiar la entrada",
  "Sound input": "Entrada de sonido",
  "Saved input": "Entrada guardada",
  "System default input": "Entrada predeterminada del sistema",
  Translation: "Traducción",
  "Side by side": "Lado a lado",
  "Show a second translation (or language) next to the first": "Mostrar una segunda traducción (o idioma) junto a la primera",
  "Open the projector output on the chosen display (Settings → Output display). Press Esc on it to close it.":
    "Abrir la salida del proyector en la pantalla elegida (Ajustes → Pantalla de salida). Pulsa Esc en ella para cerrarla.",
  "Open output": "Abrir pantalla",
  "Stacked: one above the other. Click to put them side by side.": "Apiladas: una encima de la otra. Haz clic para ponerlas lado a lado.",
  "Side by side. Click to stack them one above the other.": "Lado a lado. Haz clic para apilarlas una encima de la otra.",
  "Put translations side by side": "Poner las traducciones lado a lado",
  "Stack translations": "Apilar las traducciones",
  "Stop listening": "Dejar de escuchar",
  "Start listening": "Empezar a escuchar",
  "Starting…": "Iniciando…",
  Listening: "Escuchando",

  // Settings menu
  Settings: "Ajustes",
  "Fonts, colours, backgrounds": "Fuentes, colores, fondos",
  Style: "Estilo",
  "Models, names to listen for, search by meaning": "Modelos, nombres que reconocer, búsqueda por significado",
  "Transcription accuracy": "Precisión de la transcripción",
  "Yoruba, Igbo, Hausa and more": "Yoruba, igbo, hausa y más",
  Bibles: "Biblias",
  "Preaching languages, menus and buttons": "Idiomas de predicación, menús y botones",
  "Account, free trial, licence key": "Cuenta, prueba gratuita, clave de licencia",
  "Account and licence": "Cuenta y licencia",
  "During the service": "Durante el culto",
  "Follow reading": "Seguir la lectura",
  "Highlight the live passage as it's read and move to the next verse automatically":
    "Resaltar el pasaje en vivo mientras se lee y pasar al siguiente versículo automáticamente",
  "Follow worship songs": "Seguir los cantos",
  "Recognise songs from your library as they're sung and follow them section by section":
    "Reconocer los cantos de tu biblioteca mientras se cantan y seguirlos sección por sección",
  "Voice commands (“next verse”, “go back”)": "Comandos de voz («versículo siguiente», «atrás»)",
  "Move the live scripture when the preacher says “next verse”, “previous verse”, “go back” or “go to the first verse”":
    "Mover el pasaje en vivo cuando el predicador dice «versículo siguiente», «versículo anterior», «atrás» o «vamos al primer versículo»",
  "Show sure detections automatically, at": "Mostrar automáticamente las detecciones seguras, a partir de",
  "Automatic threshold": "Umbral automático",
  "Output display": "Pantalla de salida",
  "Automatic (second display)": "Automática (segunda pantalla)",
  primary: "principal",
  "Close the output window": "Cerrar la ventana de salida",
  "Speech engine": "Motor de voz",
  "Not benchmarked yet — runs automatically on first start": "Aún sin probar: la prueba se hace automáticamente al primer inicio",
  "Re-benchmark": "Volver a probar",

  // Dashboard
  Transcript: "Transcripción",
  Dismiss: "Descartar",
  "No sound is reaching Hearken Studio. If macOS blocked the microphone, allow it in System Settings → Privacy & Security → Microphone, then stop and start listening. Otherwise check the input isn't muted.":
    "No llega sonido a Hearken Studio. Si macOS bloqueó el micrófono, permítelo en Ajustes del Sistema → Privacidad y seguridad → Micrófono, y luego detén y vuelve a iniciar la escucha. Si no, comprueba que la entrada no esté silenciada.",
  "No sound is reaching Hearken Studio. Check the input device isn't muted and that microphone access is allowed in the system privacy settings, then stop and start listening.":
    "No llega sonido a Hearken Studio. Comprueba que el dispositivo de entrada no esté silenciado y que el acceso al micrófono esté permitido en la configuración de privacidad del sistema, y luego detén y vuelve a iniciar la escucha.",
};
