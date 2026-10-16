// Settings panels: transcription accuracy, the Bible library, more screens and sharing on the
// local network. Model labels and notes, speed notes and Bible languages come from the backend
// in English and are translated on display.

export const fr: Record<string, string> = {
  // Shared
  Download: "Télécharger",
  Remove: "Supprimer",
  Cancel: "Annuler",
  Off: "Désactivé",
  "{mb} MB": "{mb} Mo",
  "{n}%": "{n} %",
  "Loading…": "Chargement…",
  "Stop listening first": "Arrêtez d’abord l’écoute",

  // Transcription accuracy
  "Transcription accuracy and vocabulary": "Précision de la transcription et noms à reconnaître",
  Accuracy: "Précision",
  "A fast model keeps live updates quick. A larger model then re-checks each finished sentence (lines marked {check}), giving better words, names and accents in the transcript, quotes and archive.":
    "Un modèle rapide assure des mises à jour en direct réactives. Un modèle plus grand revérifie ensuite chaque phrase terminée (lignes marquées {check}), pour des mots, des noms et des accents plus justes dans la transcription, les citations et les archives.",
  "Now: live": "Actuellement : en direct",
  "accuracy pass": "passe de précision",
  "no accuracy pass": "pas de passe de précision",
  "accuracy pass off": "passe de précision désactivée",
  "This computer has no graphics chip the models can use, so the accuracy pass runs gently in the background (one low-priority thread) to keep live detection fast.":
    "Cet ordinateur n’a pas de puce graphique utilisable par les modèles : la passe de précision tourne donc doucement en arrière-plan (un seul fil d’exécution, en basse priorité) pour que la détection en direct reste rapide.",
  "It still adds roughly 0.1–0.2 s to detections; choose {off} for the fastest possible detection.":
    "Elle ajoute tout de même environ 0,1 à 0,2 s aux détections ; choisissez {off} pour une détection la plus rapide possible.",
  "Accuracy pass": "Passe de précision",
  Automatic: "Automatique",
  "(uses {model} on this computer)": "(utilise {model} sur cet ordinateur)",
  "(installed models are too slow here, so it's off)":
    "(les modèles installés sont trop lents ici, elle est donc désactivée)",
  "(download a model below)": "(téléchargez un modèle ci-dessous)",
  "Always {model}": "Toujours {model}",
  "(quick transcription only)": "(transcription rapide uniquement)",
  "{model} is too slow on this computer while listening, so references it finds would come too late. Sentences it can't reach in time keep the quick text, and its late finds aren't queued.":
    "{model} est trop lent sur cet ordinateur pendant l’écoute : les références qu’il trouve arriveraient trop tard. Les phrases qu’il ne peut pas traiter à temps gardent le texte rapide, et ses trouvailles tardives ne sont pas mises en file d’attente.",
  "Use Automatic": "Passer en automatique",
  Models: "Modèles",
  "Remove {model}? You can download it again later.":
    "Supprimer {model} ? Vous pourrez le télécharger à nouveau plus tard.",
  "✓ Installed · Remove": "✓ Installé · Supprimer",
  "On this computer: {speed}": "Sur cet ordinateur : {speed}",
  "Speed not tested yet (tested the first time it's used)":
    "Vitesse pas encore testée (elle le sera à la première utilisation)",
  "Close other heavy apps for a fair result": "Fermez les autres applications gourmandes pour un résultat fiable",
  "Testing…": "Test en cours…",
  "Test speed": "Tester la vitesse",
  "Testing speed on this computer…": "Test de la vitesse sur cet ordinateur…",
  "{received} / {total} MB": "{received} / {total} Mo",
  "Names and words to listen for": "Noms et mots à reconnaître",
  "Pastor Johnson, Grace Chapel, Shiloh,\nHoly Ghost, Ebenezer":
    "Pasteur Johnson, Chapelle de la Grâce, Silo,\nSaint-Esprit, Ében-Ézer",
  "Pastors' and members' names, the church, places, and phrases that often come out wrong. Comma or one per line. Bible book names and today's sermon plan are added automatically.":
    "Noms des pasteurs et des membres, de l’église, des lieux, et expressions souvent mal transcrites. Séparés par des virgules ou un par ligne. Les noms des livres de la Bible et le plan de la prédication du jour sont ajoutés automatiquement.",
  "Model changes apply the next time you start listening. Vocabulary applies right away.":
    "Les changements de modèle s’appliqueront au prochain démarrage de l’écoute. Les noms à reconnaître s’appliquent tout de suite.",
  "Changes apply when you start listening.": "Les changements s’appliqueront au démarrage de l’écoute.",

  // Search by meaning
  "Verses said in other words": "Versets dits avec d’autres mots",
  "Suggest live": "Suggérer en direct",
  "Finds the verse when someone paraphrases it (“don't worry about tomorrow” → Matthew 6:34), live as a suggestion and under “By meaning” in the search box. Works offline once downloaded.":
    "Retrouve le verset quand quelqu’un le paraphrase (« ne vous inquiétez pas du lendemain » → Matthieu 6:34), en direct sous forme de suggestion et sous « Par le sens » dans la recherche. Fonctionne hors ligne une fois téléchargé.",
  "Download ({mb} MB)": "Télécharger ({mb} Mo)",
  "Ready.": "Prêt.",

  // Speech models from the backend
  "Tiny (English)": "Tiny (anglais)",
  "Base (English)": "Base (anglais)",
  "Small (English)": "Small (anglais)",
  "Fastest; live updates on slow computers.": "Le plus rapide ; mises à jour en direct sur les ordinateurs lents.",
  "Live updates on most computers.": "Mises à jour en direct sur la plupart des ordinateurs.",
  "Live updates in other languages on slow computers.":
    "Mises à jour en direct dans d’autres langues sur les ordinateurs lents.",
  "Live updates in other languages, such as French and Spanish.":
    "Mises à jour en direct dans d’autres langues, comme le français et l’espagnol.",
  "Clearly more accurate than Base. Fast enough on any recent computer.":
    "Nettement plus précis que Base. Assez rapide sur tout ordinateur récent.",
  "Small for sermons in other languages. Fast enough on any recent computer.":
    "Small pour les prédications dans d’autres langues. Assez rapide sur tout ordinateur récent.",
  "Most accurate, best with accents. Needs Apple Silicon or a graphics card.":
    "Le plus précis, le meilleur avec les accents. Nécessite une puce Apple Silicon ou une carte graphique.",
  "too slow on this computer": "trop lent sur cet ordinateur",
  "{quality}, {seconds} s per 10 s of speech": "{quality}, {seconds} s pour 10 s de parole",
  "highest quality": "qualité maximale",
  "high quality": "haute qualité",
  "standard quality": "qualité standard",

  // Bible library
  "Download Bibles in other languages": "Télécharger des Bibles dans d’autres langues",
  "Bible library": "Bibliothèque de Bibles",
  "Download once (about 4–5 MB each, from eBible.org), then use offline: on screen, side by side, and on congregation phones.":
    "Téléchargez-les une fois (environ 4 à 5 Mo chacune, depuis eBible.org), puis utilisez-les hors ligne : à l’écran, côte à côte et sur les téléphones de l’assemblée.",
  "Removing…": "Suppression…",
  "Installing…": "Installation…",
  "Choose a downloaded Bible as the translation or side by side; phones can pick any of them. To listen in another language, choose it under Settings → Language.":
    "Choisissez une Bible téléchargée comme traduction ou pour l’affichage côte à côte ; les téléphones peuvent choisir n’importe laquelle. Pour écouter dans une autre langue, choisissez-la dans Réglages → Langue.",
  Yoruba: "Yoruba",
  Igbo: "Igbo",
  Hausa: "Haoussa",
  "Twi (Akuapem)": "Twi (akuapem)",
  "Twi (Asante)": "Twi (ashanti)",
  Ewe: "Éwé",
  Swahili: "Swahili",
  Portuguese: "Portugais",
  German: "Allemand",

  // More screens
  "More screens: stage display, livestream lower third": "Autres écrans : écran de scène, bandeau pour la diffusion en direct",
  Screens: "Écrans",
  "More screens": "Autres écrans",
  "Each follows what's live, on its own display. The main output keeps its own button.":
    "Chacun suit ce qui est en direct, sur son propre moniteur. La sortie principale garde son propre bouton.",
  "Screen {n}": "Écran {n}",
  "● open": "● ouvert",
  "Remove this screen": "Supprimer cet écran",
  Shows: "Affiche",
  "Stage display": "Écran de scène",
  "For the preacher or worship leader: large text, what's been read, screen status and the time.":
    "Pour le prédicateur ou le responsable de la louange : texte en grand, ce qui a été lu, l’état de l’écran et l’heure.",
  "Lower third": "Bandeau",
  "For a livestream or video switcher: the passage in a band at the bottom, over black or green to key out.":
    "Pour une diffusion en direct ou une régie vidéo : le passage dans un bandeau en bas, sur fond noir ou vert à incruster.",
  "Same as main": "Comme l’écran principal",
  "Another projector or TV showing the main look.": "Un autre projecteur ou téléviseur avec la même présentation que l’écran principal.",
  "Behind it": "Fond",
  "Black (luma key)": "Noir (luma key)",
  "Green (chroma key)": "Vert (chroma key)",
  "Theme colour": "Couleur du thème",
  Display: "Moniteur",
  Auto: "Auto",
  "Move / refresh": "Déplacer / actualiser",
  Open: "Ouvrir",
  Close: "Fermer",
  "＋ Add a screen": "＋ Ajouter un écran",
  "Press Esc on a screen to close it. One display can't show two screens at once; on a laptop alone, screens open as windows.":
    "Appuyez sur Échap sur un écran pour le fermer. Un moniteur ne peut pas afficher deux écrans à la fois ; sur un ordinateur portable seul, les écrans s’ouvrent dans des fenêtres.",

  // Share
  "Checking the name on the network…": "Vérification du nom sur le réseau…",
  "✓ Phones on this Wi-Fi can open {address}": "✓ Les téléphones connectés à ce Wi-Fi peuvent ouvrir {address}",
  "Another device on this network already uses {address}. Choose a different name.":
    "Un autre appareil de ce réseau utilise déjà {address}. Choisissez un autre nom.",
  "Couldn't announce the name; phones can use the number address below.":
    "Impossible d’annoncer le nom ; les téléphones peuvent utiliser l’adresse numérique ci-dessous.",
  Share: "Partager",
  "Share on the local network": "Partager sur le réseau local",
  "Phones, livestream overlay and remote. No internet needed.":
    "Téléphones, incrustation pour la diffusion et télécommande. Sans connexion Internet.",
  "Network name": "Nom sur le réseau",
  "Congregation: scan to follow the verses on their phone, in any translation and text size.":
    "Assemblée : un scan pour suivre les versets sur son téléphone, dans la traduction et la taille de texte de son choix.",
  "Hide QR on screen": "Masquer le QR code à l’écran",
  "Show QR on screen": "Afficher le QR code à l’écran",
  Phones: "Téléphones",
  "Livestream overlay": "Incrustation OBS",
  "OBS/vMix: add a Browser source, 1920×1080. Add ?pos=top to move it up.":
    "OBS/vMix : ajoutez une source Navigateur, en 1920×1080. Ajoutez ?pos=top pour la placer en haut.",
  Remote: "Télécommande",
  "Open on your phone and enter PIN {pin}.": "Ouvrez ce lien sur votre téléphone et saisissez le code PIN {pin}.",
  "Some Android phones can't open {local} addresses. They can use {address} instead; the QR code always works.":
    "Certains téléphones Android ne peuvent pas ouvrir les adresses {local}. Ils peuvent utiliser {address} à la place ; le QR code fonctionne toujours.",
  "Devices must be on the same Wi-Fi. If they can't connect, allow Hearken Studio in the computer's firewall.":
    "Les appareils doivent être sur le même Wi-Fi. S’ils n’arrivent pas à se connecter, autorisez Hearken Studio dans le pare-feu de l’ordinateur.",
  "Recap on phones (read or download as PDF or Word)":
    "Récapitulatif sur les téléphones (à lire ou à télécharger en PDF ou Word)",
  "Scriptures, songs and sermon notes": "Passages, chants et notes de prédication",
  "Scriptures and songs only": "Passages et chants uniquement",
  Copied: "Copié",
  Copy: "Copier",
};

export const es: Record<string, string> = {
  // Shared
  Download: "Descargar",
  Remove: "Quitar",
  Cancel: "Cancelar",
  Off: "Desactivado",
  "Loading…": "Cargando…",
  "Stop listening first": "Primero detén la escucha",

  // Transcription accuracy
  "Transcription accuracy and vocabulary": "Precisión de la transcripción y nombres que reconocer",
  Accuracy: "Precisión",
  "A fast model keeps live updates quick. A larger model then re-checks each finished sentence (lines marked {check}), giving better words, names and accents in the transcript, quotes and archive.":
    "Un modelo rápido mantiene ágiles las actualizaciones en vivo. Luego, un modelo más grande vuelve a revisar cada frase terminada (líneas marcadas con {check}) y mejora las palabras, los nombres y los acentos en la transcripción, las citas y el archivo.",
  "Now: live": "Ahora: en vivo",
  "accuracy pass": "pasada de precisión",
  "no accuracy pass": "sin pasada de precisión",
  "accuracy pass off": "pasada de precisión desactivada",
  "This computer has no graphics chip the models can use, so the accuracy pass runs gently in the background (one low-priority thread) to keep live detection fast.":
    "Esta computadora no tiene un chip gráfico que los modelos puedan usar, así que la pasada de precisión funciona con suavidad en segundo plano (un solo hilo de baja prioridad) para que la detección en vivo siga siendo rápida.",
  "It still adds roughly 0.1–0.2 s to detections; choose {off} for the fastest possible detection.":
    "Aun así, agrega unos 0,1–0,2 s a las detecciones; elige {off} para la detección más rápida posible.",
  "Accuracy pass": "Pasada de precisión",
  Automatic: "Automático",
  "(uses {model} on this computer)": "(usa {model} en esta computadora)",
  "(installed models are too slow here, so it's off)":
    "(los modelos instalados son demasiado lentos aquí, así que está desactivada)",
  "(download a model below)": "(descarga un modelo abajo)",
  "Always {model}": "Siempre {model}",
  "(quick transcription only)": "(solo transcripción rápida)",
  "{model} is too slow on this computer while listening, so references it finds would come too late. Sentences it can't reach in time keep the quick text, and its late finds aren't queued.":
    "{model} es demasiado lento en esta computadora durante la escucha, así que las referencias que encuentre llegarían demasiado tarde. Las frases que no alcance a revisar a tiempo conservan el texto rápido, y lo que encuentre tarde no se pone en la cola.",
  "Use Automatic": "Usar Automático",
  Models: "Modelos",
  "Remove {model}? You can download it again later.": "¿Quitar {model}? Puedes volver a descargarlo más tarde.",
  "✓ Installed · Remove": "✓ Instalado · Quitar",
  "On this computer: {speed}": "En esta computadora: {speed}",
  "Speed not tested yet (tested the first time it's used)":
    "Velocidad aún sin probar (se prueba la primera vez que se usa)",
  "Close other heavy apps for a fair result": "Cierra otras aplicaciones pesadas para un resultado justo",
  "Testing…": "Probando…",
  "Test speed": "Probar velocidad",
  "Testing speed on this computer…": "Probando la velocidad en esta computadora…",
  "Names and words to listen for": "Nombres y palabras que reconocer",
  "Pastor Johnson, Grace Chapel, Shiloh,\nHoly Ghost, Ebenezer":
    "Pastor Johnson, Capilla de Gracia, Silo,\nEspíritu Santo, Ebenezer",
  "Pastors' and members' names, the church, places, and phrases that often come out wrong. Comma or one per line. Bible book names and today's sermon plan are added automatically.":
    "Nombres de pastores y miembros, de la iglesia, lugares y frases que suelen salir mal. Separados por comas o uno por línea. Los nombres de los libros de la Biblia y el plan del sermón de hoy se agregan automáticamente.",
  "Model changes apply the next time you start listening. Vocabulary applies right away.":
    "Los cambios de modelo se aplican la próxima vez que empieces a escuchar. Los nombres que reconocer se aplican de inmediato.",
  "Changes apply when you start listening.": "Los cambios se aplican cuando empiezas a escuchar.",

  // Search by meaning
  "Verses said in other words": "Versículos dichos con otras palabras",
  "Suggest live": "Sugerir en vivo",
  "Finds the verse when someone paraphrases it (“don't worry about tomorrow” → Matthew 6:34), live as a suggestion and under “By meaning” in the search box. Works offline once downloaded.":
    "Encuentra el versículo cuando alguien lo parafrasea («no se preocupen por el mañana» → Mateo 6:34), en vivo como sugerencia y en «Por significado» en el cuadro de búsqueda. Funciona sin conexión una vez descargado.",
  "Download ({mb} MB)": "Descargar ({mb} MB)",
  "Ready.": "Listo.",

  // Speech models from the backend
  "Tiny (English)": "Tiny (inglés)",
  "Base (English)": "Base (inglés)",
  "Small (English)": "Small (inglés)",
  "Fastest; live updates on slow computers.": "El más rápido; actualizaciones en vivo en computadoras lentas.",
  "Live updates on most computers.": "Actualizaciones en vivo en la mayoría de las computadoras.",
  "Live updates in other languages on slow computers.":
    "Actualizaciones en vivo en otros idiomas en computadoras lentas.",
  "Live updates in other languages, such as French and Spanish.":
    "Actualizaciones en vivo en otros idiomas, como francés y español.",
  "Clearly more accurate than Base. Fast enough on any recent computer.":
    "Claramente más preciso que Base. Bastante rápido en cualquier computadora reciente.",
  "Small for sermons in other languages. Fast enough on any recent computer.":
    "Small para sermones en otros idiomas. Bastante rápido en cualquier computadora reciente.",
  "Most accurate, best with accents. Needs Apple Silicon or a graphics card.":
    "El más preciso, el mejor con acentos. Requiere Apple Silicon o una tarjeta gráfica.",
  "too slow on this computer": "demasiado lento en esta computadora",
  "{quality}, {seconds} s per 10 s of speech": "{quality}, {seconds} s por cada 10 s de voz",
  "highest quality": "máxima calidad",
  "high quality": "alta calidad",
  "standard quality": "calidad estándar",

  // Bible library
  "Download Bibles in other languages": "Descargar Biblias en otros idiomas",
  "Bible library": "Biblioteca de Biblias",
  "Download once (about 4–5 MB each, from eBible.org), then use offline: on screen, side by side, and on congregation phones.":
    "Descárgalas una vez (unos 4–5 MB cada una, desde eBible.org) y úsalas sin conexión: en pantalla, lado a lado y en los teléfonos de la congregación.",
  "Removing…": "Quitando…",
  "Installing…": "Instalando…",
  "Choose a downloaded Bible as the translation or side by side; phones can pick any of them. To listen in another language, choose it under Settings → Language.":
    "Elige una Biblia descargada como traducción o para verla lado a lado; los teléfonos pueden elegir cualquiera. Para escuchar en otro idioma, elígelo en Ajustes → Idioma.",
  Yoruba: "Yoruba",
  Igbo: "Igbo",
  Hausa: "Hausa",
  "Twi (Akuapem)": "Twi (akuapem)",
  "Twi (Asante)": "Twi (asante)",
  Ewe: "Ewe",
  Swahili: "Suajili",
  Portuguese: "Portugués",
  German: "Alemán",

  // More screens
  "More screens: stage display, livestream lower third":
    "Más pantallas: pantalla de escenario, tercio inferior para la transmisión en vivo",
  Screens: "Pantallas",
  "More screens": "Más pantallas",
  "Each follows what's live, on its own display. The main output keeps its own button.":
    "Cada una sigue lo que está en vivo, en su propio monitor. La salida principal conserva su propio botón.",
  "Screen {n}": "Pantalla {n}",
  "● open": "● abierta",
  "Remove this screen": "Quitar esta pantalla",
  Shows: "Muestra",
  "Stage display": "Pantalla de escenario",
  "For the preacher or worship leader: large text, what's been read, screen status and the time.":
    "Para el predicador o el líder de alabanza: texto grande, lo que se ha leído, el estado de la pantalla y la hora.",
  "Lower third": "Tercio inferior",
  "For a livestream or video switcher: the passage in a band at the bottom, over black or green to key out.":
    "Para una transmisión en vivo o un mezclador de video: el pasaje en una franja abajo, sobre negro o verde para recortarlo con key.",
  "Same as main": "Igual que la principal",
  "Another projector or TV showing the main look.": "Otro proyector o televisor con el mismo aspecto que la pantalla principal.",
  "Behind it": "Fondo",
  "Black (luma key)": "Negro (luma key)",
  "Green (chroma key)": "Verde (chroma key)",
  "Theme colour": "Color del tema",
  Display: "Monitor",
  Auto: "Auto",
  "Move / refresh": "Mover / actualizar",
  Open: "Abrir",
  Close: "Cerrar",
  "＋ Add a screen": "＋ Agregar una pantalla",
  "Press Esc on a screen to close it. One display can't show two screens at once; on a laptop alone, screens open as windows.":
    "Pulsa Esc en una pantalla para cerrarla. Un monitor no puede mostrar dos pantallas a la vez; en una computadora portátil sola, las pantallas se abren como ventanas.",

  // Share
  "Checking the name on the network…": "Comprobando el nombre en la red…",
  "✓ Phones on this Wi-Fi can open {address}": "✓ Los teléfonos en esta red Wi-Fi pueden abrir {address}",
  "Another device on this network already uses {address}. Choose a different name.":
    "Otro dispositivo de esta red ya usa {address}. Elige otro nombre.",
  "Couldn't announce the name; phones can use the number address below.":
    "No se pudo anunciar el nombre; los teléfonos pueden usar la dirección numérica de abajo.",
  Share: "Compartir",
  "Share on the local network": "Compartir en la red local",
  "Phones, livestream overlay and remote. No internet needed.":
    "Teléfonos, superposición para la transmisión y control remoto. Sin necesidad de internet.",
  "Network name": "Nombre en la red",
  "Congregation: scan to follow the verses on their phone, in any translation and text size.":
    "Congregación: escanean para seguir los versículos en su teléfono, en cualquier traducción y tamaño de letra.",
  "Hide QR on screen": "Ocultar el QR en pantalla",
  "Show QR on screen": "Mostrar el QR en pantalla",
  Phones: "Teléfonos",
  "Livestream overlay": "Superposición OBS",
  "OBS/vMix: add a Browser source, 1920×1080. Add ?pos=top to move it up.":
    "OBS/vMix: agrega una fuente Navegador, de 1920×1080. Agrega ?pos=top para subirla.",
  Remote: "Control remoto",
  "Open on your phone and enter PIN {pin}.": "Ábrelo en tu teléfono e ingresa el PIN {pin}.",
  "Some Android phones can't open {local} addresses. They can use {address} instead; the QR code always works.":
    "Algunos teléfonos Android no pueden abrir direcciones {local}. Pueden usar {address} en su lugar; el código QR siempre funciona.",
  "Devices must be on the same Wi-Fi. If they can't connect, allow Hearken Studio in the computer's firewall.":
    "Los dispositivos deben estar en la misma red Wi-Fi. Si no logran conectarse, permite Hearken Studio en el firewall de la computadora.",
  "Recap on phones (read or download as PDF or Word)":
    "Resumen en los teléfonos (para leer o descargar en PDF o Word)",
  "Scriptures, songs and sermon notes": "Pasajes, cantos y notas del sermón",
  "Scriptures and songs only": "Solo pasajes y cantos",
  Copied: "Copiado",
  Copy: "Copiar",
};
