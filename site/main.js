// The page is German in the HTML; English comes from this table. Everything works without storage,
// and whatever cannot run (no WebGL, reduced motion, small screen) falls back to the plain page.
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
  meterLabel: "Measured value",
  meterTop: "Task Manager",
  meterText: "3D scene on two screens",
  tick1: "No browser",
  tick2: "One 19 MB program",
  tick3: "Rust and Direct3D 11",
  tick4: "Open source under GPL-3.0",
  tick5: "No telemetry",
  tick6: "No admin rights",
  cmpKicker: "The comparison",
  cmpTitle: "less memory for the same scene.",
  cmpIntro: "Most live wallpapers run inside an embedded browser, often one per screen. Kumo draws directly with the graphics card. Measured with the same pagoda scene on the same laptop.",
  cmpChart: "Pagoda scene on two screens",
  cmpLively: "Lively with web scene",
  cmpNote: "One cube is 10 MB. “Memory” column in Task Manager, Intel Iris Xe laptop, 1440p and 1080p. The graphics card holds extra memory of its own in both cases. This figure is for a web scene: videos run differently in Lively and were not measured.",
  shareTitle: "What that means for your laptop",
  shareLabel: "Memory of the laptop",
  shareNote: "Share of the whole memory, just for the wallpaper.",
  ownTitle: "What Kumo itself needs",
  own1: "3D scene",
  ownNote: "One 1080p screen, video with hardware decoding. Videos need more than scenes, and that is part of the truth.",
  stat2Num: "0.3 MB",
  stat2: "still picture, 30 seconds after drawing",
  stat3: "control panel, only while it is open",
  stat4: "accounts, ads and telemetry",
  dayKicker: "Day and night",
  dayWord1: "Morning light",
  dayWord2: "Broad daylight",
  dayWord3: "Sunset",
  dayWord4: "Lanterns on",
  dayIntro: "The scene follows your clock. Scroll through a whole day: every picture comes straight from Kumo.",
  altEvening: "The pagoda island in the evening light",
  fanKicker: "Compare",
  fanTitle: "Pull the night across the day.",
  fanIntro: "Same camera, same moment, only the light changes. Pick a mood and move the slider.",
  dayPick: "Choose a time of day",
  altDay: "The pagoda island by day",
  altNight: "The same island at night with glowing lanterns",
  cmpSlider: "Move the comparison between day and night",
  tagNight: "Night",
  tagNoon: "Noon",
  tagMorning: "Morning",
  tagEvening: "Evening",
  pointDrag: "Drag",
  pointPick: "Choose",
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
  dlFine: "Windows 10 or 11, 64-bit, graphics card with Direct3D 11. Every release is built in public from the source, so you can check what you run.",
  s1t: "Download",
  s1: "Get the setup file from GitHub. Close Lively or Wallpaper Engine first, two wallpaper programs get in each other's way.",
  s2t: "Confirm the warning",
  s2: "Kumo is not signed yet. At the Windows warning, click “More info” and then “Run anyway”.",
  s3t: "Three steps",
  s3: "The wizard needs no admin rights. Your new wallpaper starts right away.",
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
const byId = id => document.getElementById(id);
const calm = matchMedia("(prefers-reduced-motion: reduce)").matches;
const mouse = matchMedia("(hover: hover) and (pointer: fine)").matches;

const DE = {
  title: document.title,
  dayWord1: "Morgenlicht",
  dayWord2: "Heller Tag",
  dayWord4: "Laternen an",
  pointDrag: "Ziehen",
  pointPick: "Wählen",
};
each("[data-t]", el => (DE[el.dataset.t] = el.textContent));
each("[data-t-alt]", el => (DE[el.dataset.tAlt] = el.alt));
each("[data-t-aria]", el => (DE[el.dataset.tAria] = el.getAttribute("aria-label")));

let lang = "de";
let words = DE;
const number = v => v.toLocaleString(lang, { maximumFractionDigits: 1 });

function setLang(next) {
  lang = next;
  words = next === "en" ? { ...DE, ...EN } : DE;
  document.documentElement.lang = next;
  document.title = words.title;
  each("[data-t]", el => (el.textContent = words[el.dataset.t]));
  each("[data-t-alt]", el => (el.alt = words[el.dataset.tAlt]));
  each("[data-t-aria]", el => el.setAttribute("aria-label", words[el.dataset.tAria]));
  each("[data-lang-src]", el => (el.src = el.getAttribute("src").replace(/-(de|en)\.webp/, `-${next}.webp`)));
  const button = byId("lang");
  button.textContent = next === "en" ? "DE" : "EN";
  button.setAttribute("aria-label", next === "en" ? "Auf Deutsch wechseln" : "Switch to English");
  renderShare();
}

const seen = (el, fn, rootMargin = "0px") => {
  const watcher = new IntersectionObserver(entries => {
    if (entries.some(e => e.isIntersecting)) {
      watcher.disconnect();
      fn();
    }
  }, { rootMargin });
  watcher.observe(el);
};

// neighbours in a row rise one after the other, the whole row within half a second
document.querySelectorAll("[data-r]").forEach(el => {
  const row = [...el.parentElement.children].filter(c => c.hasAttribute("data-r"));
  el.style.setProperty("--d", Math.min(row.indexOf(el), 5));
  seen(el, () => el.classList.add("in"), "0px 0px -8% 0px");
});

// memory as voxels: one cube is 10 MB, the last Kumo cube is cut to its 0.4
const fillWaffle = (id, cubes) => {
  const box = byId(id);
  for (let n = 0; n < cubes; n++) {
    const cube = document.createElement("i");
    cube.style.setProperty("--n", n);
    box.append(cube);
  }
};
fillWaffle("wafOther", 90);
fillWaffle("wafKumo", 8);

// the laptop chart: 900 and 74 MB as a share of the chosen memory size
let gigabytes = 8;
function renderShare() {
  const put = (id, mb) => {
    const percent = (mb / (gigabytes * 1024)) * 100;
    byId(id).style.setProperty("--v", `${percent}%`);
    byId(`${id}Text`).textContent = `${number(percent)} %`;
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
each(".tile", tile => {
  if (!tile.querySelector(".fill, .waffle") || calm) return;
  tile.classList.add("waiting");
  seen(tile, () => tile.classList.remove("waiting"), "0px 0px -12% 0px");
});

// the mouse light on the tiles
if (mouse) {
  each(".tile", tile =>
    tile.addEventListener("pointermove", e => {
      const r = tile.getBoundingClientRect();
      tile.style.setProperty("--mx", `${e.clientX - r.left}px`);
      tile.style.setProperty("--my", `${e.clientY - r.top}px`);
    }),
  );
}

// scrolling through one day: stacked frames of the same camera, so a fade is the light changing
const DAY_HOURS = [5.5, 7, 9, 12, 15, 17, 18, 18.6, 19.2, 20, 22, 24]; // as in tools/site_media.ps1
const day = document.querySelector(".day");
if (!calm) {
  const box = byId("dayFrames");
  const frames = [];
  seen(day, () => {
    box.replaceChildren();
    DAY_HOURS.forEach((_, i) => {
      const img = new Image(1600, 900);
      img.alt = "";
      img.decoding = "async";
      img.src = `/media/day-${String(i).padStart(2, "0")}.webp`;
      box.append(img);
      frames.push(img);
    });
    show();
  }, "90% 0px");

  const word = byId("dayWord");
  let queued = false;
  let current = -1;
  function show() {
    queued = false;
    const room = day.offsetHeight - innerHeight;
    const progress = Math.min(1, Math.max(0, -day.getBoundingClientRect().top / room));
    const place = progress * (DAY_HOURS.length - 1);
    const i = Math.min(Math.floor(place), DAY_HOURS.length - 2);
    const t = place - i;
    if (frames.length) {
      if (i !== current) {
        frames.forEach((f, n) => (f.style.opacity = n <= i ? 1 : 0));
        current = i;
      }
      frames[i + 1].style.opacity = t;
    }
    const hour = DAY_HOURS[i] + (DAY_HOURS[i + 1] - DAY_HOURS[i]) * t;
    const minutes = Math.floor(hour * 60) % 1440;
    byId("dayClock").textContent = `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;
    byId("dayBar").style.transform = `scaleX(${progress})`;
    const key = hour < 9.5 ? 1 : hour < 16.5 ? 2 : hour < 19.6 ? 3 : 4;
    word.dataset.t = `dayWord${key}`;
    word.textContent = words[`dayWord${key}`];
  }
  addEventListener("scroll", () => {
    if (!queued) {
      queued = true;
      requestAnimationFrame(show);
    }
  }, { passive: true });
  addEventListener("resize", show, { passive: true });
  show();
}

// the fan opens when it comes into view and picks what the comparison shows
const fan = byId("fan");
seen(fan, () => fan.classList.add("in"), "0px 0px -18% 0px");
const compare = byId("compare");
const range = compare.querySelector("input");
range.addEventListener("input", () => compare.style.setProperty("--x", `${range.value}%`));
const nameKey = { morning: "tagMorning", noon: "tagNoon", evening: "tagEvening", night: "tagNight" };
each(".fan-card", card =>
  card.addEventListener("click", () => {
    const right = card.dataset.img;
    const left = right === "night" ? "noon" : "night";
    byId("cmpRight").src = `/media/${right}.webp`;
    byId("cmpLeft").src = `/media/${left}.webp`;
    const label = (id, img) => {
      byId(id).dataset.t = nameKey[img];
      byId(id).textContent = words[nameKey[img]];
    };
    label("tagRight", right);
    label("tagLeft", left);
    each(".fan-card", c => c.classList.toggle("on", c === card));
  }),
);

// a small label follows the mouse over the things you can move or pick
if (mouse && !calm) {
  const pointer = byId("pointer");
  let x = 0, y = 0, shownX = 0, shownY = 0, moving = false;
  const follow = () => {
    shownX += (x - shownX) * 0.22;
    shownY += (y - shownY) * 0.22;
    pointer.style.transform = `translate3d(${shownX + 16}px, ${shownY + 18}px, 0)`;
    moving = pointer.classList.contains("on") || Math.abs(x - shownX) > 0.5;
    if (moving) requestAnimationFrame(follow);
  };
  addEventListener("pointermove", e => {
    x = e.clientX;
    y = e.clientY;
    const over = e.target.closest?.("[data-point]");
    pointer.classList.toggle("on", Boolean(over));
    if (over) {
      pointer.textContent = words[over.dataset.point === "drag" ? "pointDrag" : "pointPick"];
      if (!moving) {
        moving = true;
        shownX = x;
        shownY = y;
        requestAnimationFrame(follow);
      }
    }
  }, { passive: true });
}

// fluted glass over the hero: each rib is a small lens onto a tiny copy of the video, which is
// all the blur needs, so this stays cheap and stops as soon as the hero is out of sight
function startGlass() {
  const canvas = byId("glass");
  const hero = document.querySelector(".hero");
  const video = document.querySelector(".hero-video");
  if (calm || innerWidth <= 960) return;
  const gl = canvas.getContext("webgl", { alpha: false, antialias: false, depth: false, powerPreference: "low-power" });
  if (!gl) return;
  const shader = (type, source) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, source);
    gl.compileShader(s);
    return s;
  };
  const program = gl.createProgram();
  gl.attachShader(program, shader(gl.VERTEX_SHADER, "attribute vec2 p;varying vec2 v;void main(){v=vec2(p.x*.5+.5,.5-p.y*.5);gl_Position=vec4(p,0.,1.);}"));
  gl.attachShader(program, shader(gl.FRAGMENT_SHADER, `precision mediump float;
    uniform sampler2D tex; uniform vec4 map; uniform float ribs, shift; varying vec2 v;
    void main(){
      float x = v.x * ribs + shift, rib = floor(x), f = x - rib - .5;
      float bend = f * .9 + f * f * f * 6.;
      vec2 at = vec2((rib + .5 + bend * 1.7 - shift) / ribs, v.y);
      vec3 c = texture2D(tex, clamp(at * map.xy + map.zw, 0., 1.)).rgb;
      c *= 1. - smoothstep(.3, .5, abs(f)) * .55;
      c += exp(-pow((f + .22) * 9., 2.)) * .07;
      gl_FragColor = vec4(mix(c, vec3(.05, .035, .09), .44), 1.);
    }`));
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) return;
  gl.useProgram(program);
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  const corner = gl.getAttribLocation(program, "p");
  gl.enableVertexAttribArray(corner);
  gl.vertexAttribPointer(corner, 2, gl.FLOAT, false, 0, 0);
  gl.bindTexture(gl.TEXTURE_2D, gl.createTexture());
  for (const [k, v] of [[gl.TEXTURE_MIN_FILTER, gl.LINEAR], [gl.TEXTURE_MAG_FILTER, gl.LINEAR], [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE], [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE]]) gl.texParameteri(gl.TEXTURE_2D, k, v);
  const uniform = name => gl.getUniformLocation(program, name);

  const small = document.createElement("canvas");
  small.width = 400;
  small.height = 225;
  const paint = small.getContext("2d", { alpha: false });
  const poster = new Image();
  poster.src = video.poster;

  const fit = () => {
    const ratio = Math.min(devicePixelRatio, 1.25);
    const W = hero.clientWidth, H = hero.clientHeight, cw = canvas.clientWidth;
    canvas.width = Math.round(cw * ratio);
    canvas.height = Math.round(H * ratio);
    gl.viewport(0, 0, canvas.width, canvas.height);
    // where object-fit: cover and object-position: 62% 50% put the 16:9 video
    const scale = Math.max(W / 1600, H / 900), dw = 1600 * scale, dh = 900 * scale;
    const dx = (W - dw) * 0.62, dy = (H - dh) * 0.5;
    gl.uniform4f(uniform("map"), cw / dw, H / dh, -dx / dw, -dy / dh);
    gl.uniform1f(uniform("ribs"), Math.round(cw / 44));
  };
  let shift = 0, target = 0, last = 0, frame = 0, drawn = false;
  const draw = now => {
    frame = requestAnimationFrame(draw);
    if (now - last < 48) return;
    last = now;
    const source = video.readyState >= 2 ? video : poster.complete && poster.naturalWidth ? poster : null;
    if (!source) return;
    paint.drawImage(source, 0, 0, 400, 225);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, gl.RGB, gl.UNSIGNED_BYTE, small);
    shift += (target - shift) * 0.09;
    gl.uniform1f(uniform("shift"), shift);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    if (!drawn) {
      drawn = true;
      canvas.classList.add("on");
      document.documentElement.classList.add("has-glass");
    }
  };
  let inView = true;
  const run = () => {
    cancelAnimationFrame(frame);
    if (inView && !document.hidden) frame = requestAnimationFrame(draw);
  };
  new IntersectionObserver(e => {
    inView = e[0].isIntersecting;
    run();
  }).observe(hero);
  document.addEventListener("visibilitychange", run);
  addEventListener("resize", fit, { passive: true });
  if (mouse) hero.addEventListener("pointermove", e => (target = (e.clientX / hero.clientWidth - 0.5) * 0.7), { passive: true });
  fit();
  run();
}
startGlass();
if (calm) each(".hero-video", v => { v.removeAttribute("autoplay"); v.pause(); });

let saved = null;
try {
  saved = localStorage.getItem("kumo-lang");
} catch {}
setLang(saved || (navigator.language.toLowerCase().startsWith("de") ? "de" : "en"));
byId("lang").addEventListener("click", () => {
  const next = lang === "de" ? "en" : "de";
  setLang(next);
  try {
    localStorage.setItem("kumo-lang", next);
  } catch {}
});
