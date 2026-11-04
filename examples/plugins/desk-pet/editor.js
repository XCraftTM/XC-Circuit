// Desk Pet: an example editor plugin. A critter floats over the editor
// (xc.overlay): drag it anywhere, click it and it makes a random sound
// (xc.playSound). Timers (xc.every, xc.after) make it blink, hop around and
// nap; it worries when the running bot has an error (xc.on("botEvent")).
//
// To try it: Plugins › Install from Zip… with this folder zipped, or copy
// the folder into your plugins folder (Plugins › Open folder).

const SIZE = 96;
const SOUNDS = ["sounds/boop.wav", "sounds/chirp.wav", "sounds/squeak.wav", "sounds/purr.wav"];

const pet = {
  mood: "idle", // idle, blink, happy, sleep, worried
  until: 0, // when a happy or worried face ends (ms since start)
  lastPetted: 0,
  hop: 0, // pixels up while hopping
};
let clock = 0;

const pick = (list) => list[Math.floor(Math.random() * list.length)];
const face = () => (pet.mood === "blink" ? "blink" : pet.mood);

/** Shows a mood for a while, then goes back to idle. */
function feel(mood, ms) {
  pet.mood = mood;
  pet.until = clock + ms;
}

/** Where the pet is now: where it was moved to, or its corner. */
function where() {
  const at = xc.overlayPosition("pet");
  if (at) return at;
  const screen = xc.screen();
  return { x: screen.width - SIZE - 28, y: screen.height - SIZE - 60 };
}

/** A little jump: up, then down again. */
function jump(height = 18) {
  const start = where();
  const steps = [0.5, 0.85, 1, 0.85, 0.5, 0];
  steps.forEach((k, i) => xc.after(40 * (i + 1), () => xc.moveOverlay("pet", start.x, start.y - height * k)));
}

/** A few hops sideways along the bottom of the window. */
function wander() {
  const start = where();
  const screen = xc.screen();
  const distance = (Math.random() < 0.5 ? -1 : 1) * (40 + Math.random() * 100);
  const target = Math.max(8, Math.min(screen.width - SIZE - 8, start.x + distance));
  const steps = 12;
  for (let i = 1; i <= steps; i++) {
    const k = i / steps;
    const lift = Math.sin(k * Math.PI * 3) * 8;
    xc.after(45 * i, () => xc.moveOverlay("pet", start.x + (target - start.x) * k, start.y - Math.abs(lift)));
  }
}

xc.overlay({
  id: "pet",
  width: SIZE,
  height: SIZE,
  x: -28,
  y: -60,
  render(ui) {
    ui.image(`art/pet-${face()}.svg`, { width: SIZE, height: SIZE });
  },
  onClick() {
    pet.lastPetted = clock;
    xc.playSound(pick(SOUNDS), { volume: 0.6 });
    feel("happy", 1500);
    jump();
  },
});

// The pet's clock: blinking, napping, wandering.
xc.every(250, () => {
  clock += 250;
  if (pet.until && clock >= pet.until) {
    pet.until = 0;
    pet.mood = "idle";
  }
  if (pet.mood === "blink") pet.mood = "idle";
  // Left alone for two minutes, it naps until you click it.
  if (pet.mood === "idle" && clock - pet.lastPetted > 120000) pet.mood = "sleep";
  if (pet.mood === "idle" && Math.random() < 0.03) pet.mood = "blink";
  if (pet.mood === "idle" && Math.random() < 0.004) wander();
});

xc.on("botEvent", (event) => {
  if (event.type === "block" && event.error) feel("worried", 3000);
  if (event.type === "started") feel("happy", 1200);
});

xc.on("save", () => {
  if (pet.mood !== "sleep") jump(10);
});

xc.command({
  id: "desk-pet.come-back",
  title: "Desk Pet: come back",
  description: "Puts the pet back in its corner",
  run() {
    const screen = xc.screen();
    xc.moveOverlay("pet", screen.width - SIZE - 28, screen.height - SIZE - 60);
    feel("happy", 1000);
  },
});
