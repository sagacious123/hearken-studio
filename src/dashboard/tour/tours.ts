// The guided tours: a welcome tour of the whole screen, and a short one for each panel the
// first time it opens. Steps point at elements by their data-tour name; text is English and
// translated through t() like the rest of the app (src/lib/i18n/tours.ts).
import { useDashboard } from "../store";

export type TourId = "basics" | "settings" | "share" | "screens" | "style" | "songs" | "archive" | "plan";

export interface Step {
  /** data-tour name of the element to light up; none shows the card in the middle. */
  target?: string;
  title: string;
  body: string;
  /** Opens what the step needs (a menu, a tab) when its element isn't on screen yet. */
  prepare?: () => void;
}

export interface Tour {
  id: TourId;
  /** In the list of tours. */
  name: string;
  summary: string;
  steps: Step[];
}

const el = (name: string) => document.querySelector<HTMLElement>(`[data-tour="${name}"]`);
/** Clicks `button` unless `shown` is already on screen (opening a menu or a tab). */
const open = (shown: string, button: string) => () => {
  if (!el(shown)) el(button)?.click();
};

export const TOURS: Tour[] = [
  {
    id: "basics",
    name: "The basics",
    summary: "A one-minute tour of the screen you’ll use every Sunday.",
    steps: [
      { title: "Welcome to Hearken", body: "Here’s the screen you’ll use every Sunday, in a minute. Use the arrow keys or the buttons to move through it." },
      { target: "listen", title: "Start listening", body: "Press this when the service begins. Hearken listens to the sermon and finds every scripture as it’s said, read or quoted." },
      { target: "input", title: "Your sound", body: "Choose the sound desk or microphone here. The little meter moves when someone speaks: if it stays still, check the cable or the desk’s send." },
      { target: "translation", title: "The Bible on screen", body: "Pick the translation the congregation sees. Add a second one with + to show two side by side." },
      { target: "detections", title: "Every verse, as it’s heard", body: "Each scripture Hearken hears lands here with how sure it is. Click a card to preview it, or double-click to put it straight on screen." },
      { target: "monitors", title: "Preview, then live", body: "Preview is only for you. When it looks right, press Go live, or Enter, and it’s on the screen. Black and Clear are on the right, for a hurry." },
      { target: "finder", title: "Find any passage", body: "Type a reference like “jn 3 16” or words like “love is patient”, or pick the book, chapter and verse yourself." },
      { target: "transcript", title: "What’s being said", body: "The sermon, line by line, with the verse each line led to. Checked lines have been re-heard by a larger model." },
      { target: "sidebar", title: "Plan, songs and archive", body: "Paste the pastor’s notes so the day’s verses are ready, add your worship songs, and find past services and their recaps." },
      { target: "output", title: "Open the screen", body: "This opens the verses on your projector or TV. Press Esc on it to close it. You can open it again any time." },
      { target: "settings", title: "Everything else", body: "Share sends verses to phones and livestreams, Screens adds a stage display, and Settings holds the style, Bibles, languages and more." },
      { title: "You’re ready for Sunday", body: "That’s the whole screen. Each panel shows a short tour the first time you open it, and every tour is under Settings → Guided tours." },
    ],
  },
  {
    id: "settings",
    name: "Settings",
    summary: "The look, Bibles, languages and what happens during the service.",
    steps: [
      { target: "settings-panels", prepare: open("settings-panels", "settings"), title: "Make it yours", body: "Style changes fonts and backgrounds. Bibles adds Yoruba, Igbo, Hausa and more. Language sets what’s preached and the language of the menus." },
      { target: "settings-service", prepare: open("settings-panels", "settings"), title: "During the service", body: "Let the words light up as they’re read, follow the worship songs, and let the preacher say “next verse”. Hearken can also show the verses it’s very sure of by itself." },
      { target: "settings-display", prepare: open("settings-panels", "settings"), title: "Which screen", body: "Choose the display the verses open on. Automatic picks the second display." },
    ],
  },
  {
    id: "share",
    name: "Phones and livestream",
    summary: "Members follow on their phones, a phone remote, and an overlay for OBS or vMix.",
    steps: [
      { target: "share-toggle", prepare: open("share-toggle", "share"), title: "Share on the church Wi-Fi", body: "Turn this on to send the live verse to phones, a livestream overlay and a phone remote. No internet is needed, only the same Wi-Fi." },
      { target: "share-qr", prepare: open("share-toggle", "share"), title: "Members follow on their phones", body: "Show this QR code on the screen. Members scan it and read along in their own translation and text size, with no app to install." },
    ],
  },
  {
    id: "screens",
    name: "More screens",
    summary: "A stage display for the preacher, and a lower third for video.",
    steps: [
      { target: "screens-add", prepare: open("screens-add", "screens"), title: "More screens", body: "Add a stage display for the preacher, a lower third for the livestream, or another projector. Each follows what’s live on its own display." },
    ],
  },
  {
    id: "style",
    name: "Style",
    summary: "Fonts, colours and backgrounds for the screen.",
    steps: [
      { target: "style-start", prepare: () => useDashboard.getState().setStyleOpen(true), title: "Start from a look", body: "Pick a ready-made look, then change anything. Your changes show in Preview only, until you press Apply to screen." },
      { target: "style-background", prepare: () => useDashboard.getState().setStyleOpen(true), title: "Backgrounds", body: "A plain colour, one of the sample pictures, or your own photos. Darken and soften a picture so the words stay easy to read." },
      { target: "style-verse", prepare: () => useDashboard.getState().setStyleOpen(true), title: "The words", body: "Any font on this computer, its size, colour, outline and shadow, and the colour words light up in as they’re read." },
    ],
  },
  {
    id: "plan",
    name: "The sermon plan",
    summary: "Line up the day’s passages before the service.",
    steps: [
      { target: "plan-notes", prepare: open("plan-notes", "tab-plan"), title: "Paste the sermon notes", body: "Paste the pastor’s notes or verse list as they are. Every reference in them is found for you." },
      { target: "plan-line-up", prepare: open("plan-notes", "tab-plan"), title: "Ready to show", body: "Line up references, and each passage is one click from the screen. Hearken is also more sure of them when they’re spoken." },
    ],
  },
  {
    id: "songs",
    name: "Worship songs",
    summary: "Lyrics that follow the singers.",
    steps: [
      { target: "songs-add", prepare: open("songs-add", "tab-songs"), title: "Your song library", body: "Add a song by pasting its lyrics, or import .txt files. While the room sings, Hearken follows the song part by part." },
      { target: "songs-ccli", prepare: open("songs-add", "tab-songs"), title: "CCLI reporting", body: "Every song shown is counted. Export the last 90 days for your CCLI report in one click." },
    ],
  },
  {
    id: "archive",
    name: "The archive",
    summary: "Past sermons, recaps and the year at a glance.",
    steps: [
      { target: "archive-search", prepare: open("archive-search", "tab-archive"), title: "Every service, searchable", body: "Search everything said and shown in past services, by passage or by words like “mustard seed”." },
      { target: "archive-services", prepare: open("archive-search", "tab-archive"), title: "Sermon recaps", body: "Save any service’s recap, with its notes, scriptures and songs, as a PDF or Word document to share." },
      { target: "archive-insights", prepare: open("archive-search", "tab-archive"), title: "The year at a glance", body: "See the books and passages preached most, and the songs sung most, year by year." },
    ],
  },
];

export const tourById = (id: TourId) => TOURS.find((t) => t.id === id)!;

/**
 * Panels that show their tour the first time they open, for churches new to Hearken: the
 * element whose appearance means the panel is open. The plan's tab is open from the start, so
 * the welcome tour covers it instead.
 */
export const FIRST_OPEN: [TourId, string][] = [
  ["settings", "settings-panels"],
  ["share", "share-toggle"],
  ["screens", "screens-add"],
  ["style", "style-start"],
  ["songs", "songs-add"],
  ["archive", "archive-search"],
];
