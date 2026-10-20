export interface Rules {
  duration_seconds: number;
  words_per_minute: number;
  language: string;
  difficulty: string;
  style: string;
  instructions: string;
  optional_ending: boolean;
}
export interface Source {
  id: string;
  name: string;
  mime: string;
  text: string;
  blob: string | null;
  digest: string;
  extracted: string;
}
export interface Card {
  title: string;
  text: string;
  tags: string[];
  source_ids: string[];
}
export interface Compiled {
  identity: string;
  facts: string[];
  boundaries: string[];
  cards: Card[];
  questions: string[];
  unknowns: string[];
}
export interface Active {
  revision: string;
  compiled: Compiled;
  rules: Rules;
  goal: string;
}
export interface Profile {
  id: string;
  name: string;
  goal: string;
  rules: Rules;
  sources: Source[];
  clarification: string;
  draft: Compiled | null;
  active: Active | null;
}
export interface Connection {
  id: string;
  name: string;
  kind: "openai" | "compatible" | "gemini" | "anthropic";
  base_url: string;
  model: string;
  vision: boolean;
  pdf: boolean;
  parameters: Record<string, unknown> | null;
}
export interface Monitor {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
}
export interface Region {
  monitor: Monitor;
  x: number;
  y: number;
  width: number;
  height: number;
}
export interface Overlay {
  x: number;
  y: number;
  width: number;
  height: number;
  font_size: number;
  font_family: string;
}
export interface Settings {
  active_profile: string | null;
  preparation_connection: string | null;
  answer_connections: string[];
  race: boolean;
  timeout_seconds: number;
  region: Region | null;
  overlay: Overlay;
  capture_shortcut: string;
  toggle_shortcut: string;
}
export interface Caption {
  request_id: string;
  profile_id: string;
  revision: string;
  question: string;
  answer: string;
  status: string;
  detail: string;
  elapsed_ms: number;
  cached: boolean;
}
export interface Job {
  id: string;
  profile_id: string;
  kind: string;
  status: string;
  step: number;
  total: number;
  detail: string;
}
export interface Answer {
  id: string;
  profile_id: string;
  revision: string;
  question: string;
  text: string;
  prepared: boolean;
  enabled: boolean;
  origin: string;
}
export interface Snapshot {
  profiles: Profile[];
  connections: Connection[];
  has_keys: Record<string, boolean>;
  settings: Settings;
  caption: Caption;
  ocr_status: string;
  ocr_ready: boolean;
  windows: boolean;
  visible: boolean;
  jobs: Job[];
  shortcut_error?: string | null;
}
export const defaultRules: Rules = {
  duration_seconds: 60,
  words_per_minute: 90,
  language: "English",
  difficulty:
    "Simple vocabulary, approximately 4000-word learner level; preserve necessary technical terms.",
  style:
    "Professional spoken English. Short sentences and clear connecting words. Commas may mark speaking breaks; periods mark pauses.",
  instructions: "",
  optional_ending: true,
};
export const emptyCompiled: Compiled = {
  identity: "",
  facts: [],
  boundaries: [],
  cards: [],
  questions: [],
  unknowns: [],
};
