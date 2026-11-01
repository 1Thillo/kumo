// The page is German in the HTML; English comes from this table. Everything works without storage.
const EN = {
  title: "Kumo – live wallpapers that leave your PC alone",
  skip: "Skip to content",
  navLabel: "Main menu",
  navCompare: "Compare",
  navDay: "Day and night",
  navFeatures: "Features",
  navDownload: "Download",
  eyebrow: "kumo, Japanese for cloud",
  heroTitle1: "Your desktop comes alive.",
  heroTitle2: "Your PC can breathe.",
  heroLead: "Kumo shows 3D scenes, videos and pictures behind your desktop icons. No browser running in the background, just one 19 MB program.",
  ctaDownload: "Download free for Windows",
  ctaSource: "View the source",
  heroFine: "Windows 10 and 11 · no admin rights · open source under GPL-3.0",
  statsLabel: "Key figures",
  stat1: "3D scene on two screens",
  stat2Num: "0.3 MB",
  stat2: "still picture, 30 seconds after drawing",
  stat3: "control panel, only while it is open",
  stat4: "accounts, ads and telemetry",
  cmpKicker: "The comparison",
  cmpTitle: "The same scene. A twelfth of the memory.",
  cmpIntro: "Most live wallpapers run inside an embedded browser, often one per screen. Kumo draws directly with the graphics card. We measured the same pagoda scene on the same laptop.",
  cmpChart: "Pagoda scene on two screens (1440p and 1080p)",
  cmpLively: "Lively with web scene",
  cmpNote: "“Memory” column in Task Manager, Intel Iris Xe laptop. The graphics card holds extra memory of its own in both cases. This figure is for a web scene: videos run differently in Lively and were not measured.",
  shareTitle: "What that means for your laptop",
  shareLabel: "Memory of the laptop",
  shareNote: "Share of the whole memory, just for the wallpaper.",
  ownTitle: "What Kumo itself needs",
  own1: "3D scene",
  ownNote: "One 1080p screen, video with hardware decoding. Videos need more than scenes, and that is part of the truth.",
  dayKicker: "Day and night",
  dayTitle: "The day moves across your screen.",
  dayIntro: "The scene follows your clock: morning light, sunset, lanterns at night. You can also pick a fixed time or a time-lapse. Drag the slider and compare for yourself.",
  altDay: "The pagoda island by day",
  altNight: "The same island at night with glowing lanterns",
  cmpSlider: "Move the comparison between day and night",
  dayPick: "Choose a time of day",
  tagNight: "Night",
  tagNoon: "Noon",
  tagMorning: "Morning",
  tagEvening: "Evening",
  clipLabel: "The scene in time-lapse over 24 hours",
  clipCaption: "24 hours in 20 seconds. Rendered directly by Kumo, nothing retouched.",
  featKicker: "Features",
  featTitle: "Everything a wallpaper needs. Nothing that gets in the way.",
  f1t: "Scenes, videos, pictures",
  f1: "3D worlds, MP4, WebM, MKV and whatever else Windows can play. Drag a file into the window and you are done.",
  f2t: "Every screen on its own",
  f2: "The same everywhere, something different per screen, or nothing at all on one.",
  f3t: "Pauses by itself",
  f3: "Covered screen, battery, energy saver, locked PC, Remote Desktop: Kumo gets out of the way.",
  f4t: "Your rules",
  f4: "15, 30 or 60 frames per second, render resolution, edge smoothing and playback speed.",
  f5t: "Made for everyone",
  f5: "Three-step setup, German and English, light and dark, usable with the keyboard.",
  f6t: "Nothing in the background",
  f6: "No account, no ads, no telemetry. The control panel exits when you close it.",
  appKicker: "Interface",
  appTitle: "As tidy as Windows itself.",
  altLibrary: "Kumo library with the pagoda scene",
  altScreens: "Setting up two screens one by one",
  altScene: "Adjusting the time of day, camera and cherry blossoms of the scene",
  howKicker: "Under the hood",
  howTitle: "Why Kumo needs so little.",
  h1t: "No browser",
  h1: "Kumo is written in Rust and draws directly with Direct3D 11. It starts no web engine, not even one per screen.",
  h2t: "One graphics device, one process",
  h2: "All screens share models and shaders. Screens of the same shape even share one picture.",
  h3t: "Video in hardware",
  h3: "Frames stay in the graphics card's decoder, and audio is never decoded. A video nobody can see costs nothing.",
  h4t: "Sleeping, not waiting",
  h4: "When nobody is looking, Kumo frees its render targets and uses neither CPU nor GPU. After 30 seconds the decoder goes too.",
  dlKicker: "Download",
  dlTitle: "Running in a minute.",
  s1t: "Download",
  s1: "Get the setup file from GitHub. Close Lively or Wallpaper Engine first, two wallpaper programs get in each other's way.",
  s2t: "Confirm the warning",
  s2: "Kumo is not signed yet. At the Windows warning, click “More info” and then “Run anyway”.",
  s3t: "Three steps",
  s3: "The wizard needs no admin rights. Your new wallpaper starts right away.",
  dlFine: "Windows 10 or 11, 64-bit, graphics card with Direct3D 11. Every release is built in public from the source, so you can check what you run.",
  q1: "What does Kumo cost?",
  a1: "Nothing. Kumo is free software under the GPL-3.0, with no account and no ads.",
  q2: "Does Kumo slow down my games?",
  a2: "A screen covered by a maximized window is not drawn. When no screen can be seen, Kumo pauses completely.",
  q3: "Why does Windows warn me at the start?",
  a3: "Windows warns about programs without a known signature. That says nothing about what is inside: the source is open.",
  q4: "How do I remove Kumo?",
  a4: "In Windows Settings under Apps, or in Kumo under Settings. Your normal Windows wallpaper comes back.",
  footLicense: "Free software under GPL-3.0",
  footLabel: "Legal",
  footImprint: "Legal notice",
  footPrivacy: "Privacy",
};

const each = (selector, fn) => document.querySelectorAll(selector).forEach(fn);
const DE = { title: document.title };
each("[data-t]", el => (DE[el.dataset.t] = el.textContent));
each("[data-t-alt]", el => (DE[el.dataset.tAlt] = el.alt));
each("[data-t-aria]", el => (DE[el.dataset.tAria] = el.getAttribute("aria-label")));

let lang = "de";
const number = v => v.toLocaleString(lang, { maximumFractionDigits: 1 });

function setLang(next) {
  lang = next;
  const words = next === "en" ? { ...DE, ...EN } : DE;
  document.documentElement.lang = next;
  document.title = words.title;
  each("[data-t]", el => (el.textContent = words[el.dataset.t]));
  each("[data-t-alt]", el => (el.alt = words[el.dataset.tAlt]));
  each("[data-t-aria]", el => el.setAttribute("aria-label", words[el.dataset.tAria]));
  each("[data-lang-src]", el => (el.src = el.getAttribute("src").replace(/-(de|en)\.webp/, `-${next}.webp`)));
  const button = document.getElementById("lang");
  if (button) {
    button.textContent = next === "en" ? "DE" : "EN";
    button.setAttribute("aria-label", next === "en" ? "Auf Deutsch wechseln" : "Switch to English");
  }
  renderShare();
}

// the laptop chart: 900 and 74 MB as a share of the chosen memory size
let gigabytes = 8;
function renderShare() {
  const other = document.getElementById("shareOther");
  if (!other) return;
  const put = (id, mb) => {
    const percent = (mb / (gigabytes * 1024)) * 100;
    document.getElementById(id).style.setProperty("--v", `${percent}%`);
    document.getElementById(`${id}Text`).textContent = `${number(percent)} %`;
  };
  put("shareOther", 900);
  put("shareKumo", 74);
}
each("[data-gb]", button =>
  button.addEventListener("click", () => {
    gigabytes = Number(button.dataset.gb);
    each("[data-gb]", b => b.classList.toggle("on", b === button));
    renderShare();
  }),
);

// the day and night slider
const compare = document.getElementById("compare");
if (compare) {
  const range = compare.querySelector("input");
  const move = () => compare.style.setProperty("--x", `${range.value}%`);
  range.addEventListener("input", move);
  each(".chips button", chip =>
    chip.addEventListener("click", () => {
      document.getElementById("cmpDay").src = `/media/${chip.dataset.img}.webp`;
      const tag = document.getElementById("cmpTag");
      tag.dataset.t = chip.dataset.t;
      tag.textContent = chip.textContent;
      each(".chips button", c => c.classList.toggle("on", c === chip));
    }),
  );
}

// bars grow when they come into view, the clip only loads then
const calm = matchMedia("(prefers-reduced-motion: reduce)").matches;
const seen = (el, fn, rootMargin = "0px") => {
  const watcher = new IntersectionObserver(entries => {
    if (entries.some(e => e.isIntersecting)) {
      watcher.disconnect();
      fn();
    }
  }, { rootMargin });
  watcher.observe(el);
};
each(".card", card => {
  if (!card.querySelector(".fill") || calm) return;
  card.classList.add("waiting");
  seen(card, () => card.classList.remove("waiting"));
});
const clip = document.getElementById("dayClip");
if (clip) {
  if (calm) clip.controls = true;
  else seen(clip, () => clip.play().catch(() => (clip.controls = true)), "200px");
}
if (calm) each(".hero-video", v => { v.removeAttribute("autoplay"); v.pause(); });

let saved = null;
try {
  saved = localStorage.getItem("kumo-lang");
} catch {}
const wanted = saved || (navigator.language.toLowerCase().startsWith("de") ? "de" : "en");
setLang(document.documentElement.dataset.only || wanted);
document.getElementById("lang")?.addEventListener("click", () => {
  const next = lang === "de" ? "en" : "de";
  setLang(next);
  try {
    localStorage.setItem("kumo-lang", next);
  } catch {}
});
