// How the mascot dances: from the genre tag, then words in the title/album,
// then what the music sounds like (tempo, loudness). "bop" is the standard nod.

export type Mood = "bop" | "mosh" | "pogo" | "cute" | "party" | "salsa" | "groove" | "chill";

export const MOODS: { id: Mood; label: string; genres: string }[] = [
  { id: "bop", label: "Bop", genres: "Standard nod on the beat" },
  { id: "mosh", label: "Mosh", genres: "Rock, metal, nu metal, hardcore" },
  { id: "pogo", label: "Pogo", genres: "Punk, ska, pop punk" },
  { id: "cute", label: "Cute", genres: "Pop, K-pop, indie pop" },
  { id: "party", label: "Party", genres: "Electronic, house, techno, EDM, disco" },
  { id: "salsa", label: "Salsa", genres: "Salsa, bachata, cumbia, merengue, latin" },
  { id: "groove", label: "Groove", genres: "Hip-hop, rap, R&B, funk, reggaeton" },
  { id: "chill", label: "Chill", genres: "Jazz, blues, classical, ambient, lo-fi" },
];

// first match wins, so specific words come before generic ones
const RULES: [RegExp, Mood][] = [
  [/punk|ska|emo/, "pogo"],
  [/metal|rock|grunge|hardcore|core\b|thrash|doom|alternative/, "mosh"],
  [/salsa|bachata|cumbia|merengue|tropical|son cubano|mambo|latin(?!.*(pop|urban|trap))/, "salsa"],
  [/hip.?hop|rap|trap|r&b|rnb|funk|soul|reggaeton|urbano|latin urban|dancehall|drill|grime/, "groove"],
  [/electro|edm|house|techno|trance|dance|disco|dubstep|drum.?n.?bass|dnb|club|garage|synthwave/, "party"],
  [/jazz|blues|classical|orchestra|piano|ambient|lo.?fi|chill|acoustic|folk|bossa|instrumental|soundtrack|new age|country/, "chill"],
  [/pop|k-?pop|j-?pop|idol|indie/, "cute"],
];

// words that describe a version of a song, whatever the genre
const DESCRIPTION: [RegExp, Mood][] = [
  [/\b(remix|club mix|extended mix|edit|vip|bootleg)\b/, "party"],
  [/\b(acoustic|unplugged|piano version|lullaby|orchestral|strings)\b/, "chill"],
];

export interface Listening {
  /** Loudness averaged over the track, 0..1. */
  energy: number;
  bpm: number;
  /** Seconds analysed: the sound only decides after a few. */
  heard: number;
}

export function moodFor(genre: string | null | undefined, description = "", sound?: Listening): Mood {
  const g = (genre ?? "").toLowerCase();
  for (const [re, mood] of RULES) if (re.test(g)) return mood;
  const d = description.toLowerCase();
  for (const [re, mood] of DESCRIPTION) if (re.test(d)) return mood;
  if (sound && sound.heard > 8) {
    if (sound.bpm >= 118 && sound.energy > 0.42) return "party";
    if (sound.energy < 0.16 || (sound.bpm < 90 && sound.energy < 0.28)) return "chill";
  }
  return "bop";
}
