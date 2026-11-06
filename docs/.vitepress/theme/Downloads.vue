<script setup lang="ts">
// Download buttons for the latest release on GitHub, by system. The release
// is asked for in the browser (the site is built before a release is
// published); until it answers, or if it can't, every button opens the
// release page instead.
import { computed, onMounted, ref } from "vue";

const REPO = "XCraftTM/XC-Circuit";
const RELEASE_PAGE = `https://github.com/${REPO}/releases/latest`;

type Asset = { name: string; size: number; browser_download_url: string };
type Release = { tag_name: string; published_at: string; html_url: string; assets: Asset[] };

type Item = { label: string; detail: string; match: RegExp };
type Group = { title: string; os: "windows" | "linux"; note: string; items: Item[] };

const GROUPS: Group[] = [
  {
    title: "Windows",
    os: "windows",
    note: "Windows 10 and 11. The Setup installs for you only, no administrator needed.",
    items: [
      { label: "Setup", detail: "x64 · most PCs", match: /-windows-x64-setup\.exe$/ },
      { label: "Setup", detail: "ARM64 · Snapdragon laptops", match: /-windows-arm64-setup\.exe$/ },
      { label: "Portable .zip", detail: "x64 · unpack and start xc-circuit.exe", match: /-windows-x64\.zip$/ },
      { label: "Portable .zip", detail: "ARM64", match: /-windows-arm64\.zip$/ },
    ],
  },
  {
    title: "Linux",
    os: "linux",
    note: "Any distribution with the AppImage; or the package for yours.",
    items: [
      { label: "AppImage", detail: "x64 · any distribution", match: /-linux-x86_64\.AppImage$/ },
      { label: "AppImage", detail: "ARM64", match: /-linux-aarch64\.AppImage$/ },
      { label: ".deb", detail: "x64 · Ubuntu, Debian, Mint, Pop!_OS", match: /-linux-x86_64\.deb$/ },
      { label: ".deb", detail: "ARM64", match: /-linux-aarch64\.deb$/ },
      { label: ".rpm", detail: "x64 · Fedora, openSUSE, RHEL", match: /-linux-x86_64\.rpm$/ },
      { label: ".rpm", detail: "ARM64", match: /-linux-aarch64\.rpm$/ },
      { label: ".tar.gz", detail: "x64 · unpack and start xc-circuit", match: /-linux-x64\.tar\.gz$/ },
      { label: ".tar.gz", detail: "ARM64", match: /-linux-arm64\.tar\.gz$/ },
    ],
  },
];

const release = ref<Release | null>(null);
const failed = ref(false);
const os = ref<"windows" | "linux" | null>(null);

onMounted(async () => {
  const ua = navigator.userAgent;
  os.value = /Windows/i.test(ua) ? "windows" : /Linux|X11/i.test(ua) && !/Android/i.test(ua) ? "linux" : null;
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { accept: "application/vnd.github+json" } });
    if (!res.ok) throw new Error(String(res.status));
    release.value = await res.json();
  } catch {
    failed.value = true;
  }
});

function asset(item: Item): Asset | undefined {
  return release.value?.assets.find((a) => item.match.test(a.name));
}

function href(item: Item): string {
  return asset(item)?.browser_download_url ?? RELEASE_PAGE;
}

function size(item: Item): string {
  const a = asset(item);
  return a ? `${Math.max(1, Math.round(a.size / 1048576))} MB` : "";
}

const version = computed(() => release.value?.tag_name.replace(/^v/, "") ?? "");
const date = computed(() => (release.value ? new Date(release.value.published_at).toLocaleDateString(undefined, { year: "numeric", month: "long", day: "numeric" }) : ""));
// The one most people on this computer want.
const suggested = computed(() => (os.value === "windows" ? GROUPS[0].items[0] : os.value === "linux" ? GROUPS[1].items[0] : null));
const groups = computed(() => (os.value === "linux" ? [GROUPS[1], GROUPS[0]] : GROUPS));
</script>

<template>
  <div class="dl">
    <p class="dl-version">
      <template v-if="release">
        Latest version <strong>{{ version }}</strong> · {{ date }} ·
        <a :href="release.html_url">what's new</a>
      </template>
      <template v-else-if="failed">
        GitHub didn't answer: the buttons open the <a :href="RELEASE_PAGE">latest release</a>, where every file is listed.
      </template>
      <template v-else>Looking up the latest version…</template>
    </p>

    <a v-if="suggested" class="dl-main" :href="href(suggested)">
      <span class="dl-main-title">Download for {{ os === "windows" ? "Windows" : "Linux" }}</span>
      <span class="dl-main-detail">{{ suggested.label }} · {{ suggested.detail }}<template v-if="size(suggested)"> · {{ size(suggested) }}</template></span>
    </a>

    <section v-for="g in groups" :key="g.title" class="dl-group">
      <h3 class="dl-group-title">{{ g.title }}</h3>
      <p class="dl-note">{{ g.note }}</p>
      <div class="dl-grid">
        <a v-for="item in g.items" :key="item.match.source" class="dl-button" :href="href(item)">
          <span class="dl-label">{{ item.label }}</span>
          <span class="dl-detail">{{ item.detail }}</span>
          <span v-if="size(item)" class="dl-size">{{ size(item) }}</span>
        </a>
      </div>
    </section>
  </div>
</template>

<style scoped>
.dl {
  margin: 24px 0 8px;
}
.dl-version {
  color: var(--vp-c-text-2);
  font-size: 14px;
}
.dl-main {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 16px 0 28px;
  padding: 16px 22px;
  border-radius: 12px;
  /* The home page's main button: aqua with white text, Run green in the dark theme. */
  background: var(--vp-c-brand-3);
  color: #fff;
  text-decoration: none;
  max-width: 420px;
  transition: background 0.2s;
}
/* Its own colours on hover too: the docs turn a link's text aqua then. */
.dl-main:hover {
  background: var(--vp-c-brand-1);
  color: #fff;
}
.dark .dl-main {
  background: #3dff6e;
  color: #04210d;
}
.dark .dl-main:hover {
  background: #6bff8f;
  color: #04210d;
}
.dl-main-title {
  font-size: 18px;
  font-weight: 600;
}
.dl-main-detail {
  font-size: 13px;
  opacity: 0.85;
}
.dl-group {
  margin-top: 24px;
}
.dl-group-title {
  margin: 0 0 4px;
  border-top: none;
  padding-top: 0;
}
.dl-note {
  margin: 0 0 12px;
  color: var(--vp-c-text-2);
  font-size: 14px;
}
.dl-grid {
  display: grid;
  /* In pairs: x64 on the left, ARM64 on the right. */
  grid-template-columns: repeat(2, 1fr);
  gap: 10px;
}
@media (max-width: 480px) {
  .dl-grid {
    grid-template-columns: 1fr;
  }
}
.dl-button {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 12px 14px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 10px;
  background: var(--vp-c-bg-soft);
  color: var(--vp-c-text-1);
  text-decoration: none;
  transition: border-color 0.2s, background 0.2s;
}
.dl-button:hover {
  border-color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-text-1);
}
.dl-label {
  font-weight: 600;
}
.dl-detail {
  font-size: 13px;
  color: var(--vp-c-text-2);
}
.dl-size {
  font-size: 12px;
  color: var(--vp-c-text-3);
}
</style>
