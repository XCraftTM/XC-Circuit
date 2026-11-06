// On New Post in a running bot, with the sites' answers stood in for
// (fixtures/mock-fetch.js): YouTube (a handle, Shorts skipped), Bluesky,
// Twitch (with its keys), Steam and an RSS feed. The first check announces
// nothing (unless asked to); a post that appears later is announced once,
// also after a restart; a source that fails is reported once.
// Usage: node runtime/test/alerts.js
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const assert = require("assert");
const { spawn } = require("child_process");

const root = path.join(__dirname, "..", "..");
const blocks = {};
for (const sub of ["web", "debug"]) {
  const folder = path.join(root, "plugins", "xc.core", "blocks", sub);
  for (const f of fs.readdirSync(folder)) blocks[require(path.join(folder, f)).id] = { source: fs.readFileSync(path.join(folder, f), "utf8"), file: path.join(folder, f) };
}

const dir = fs.mkdtempSync(path.join(os.tmpdir(), "xc-alerts-"));
const answersFile = path.join(dir, "answers.json");
const rssItem = (n) => `<item><title>Post ${n}</title><link>https://blog.example/${n}</link><guid>post-${n}</guid><pubDate>Mon, 0${n} Nov 2026 10:00:00 GMT</pubDate><description><![CDATA[<p>Text <b>${n}</b> &amp; more</p>]]></description></item>`;
const rss = (...ns) => `<?xml version="1.0"?><rss><channel><title>Blog</title>${ns.map(rssItem).join("")}</channel></rss>`;
const ytEntry = (id, title, short) =>
  `<entry><id>yt:video:${id}</id><yt:videoId>${id}</yt:videoId><title>${title}</title><link rel="alternate" href="https://www.youtube.com/${short ? "shorts/" : "watch?v="}${id}"/><author><name>Chan</name></author><published>2026-11-0${id.length}T10:00:00+00:00</published><media:group><media:thumbnail url="https://i.ytimg.com/vi/${id}/hq.jpg" width="480" height="360"/><media:description>About ${title}</media:description></media:group></entry>`;
const answers = {
  "https://www.youtube.com/@chan": { body: '<html>"externalId":"UCabcdefghijklmnopqrstuv"</html>' },
  "https://www.youtube.com/feeds/videos.xml?channel_id=UCabcdefghijklmnopqrstuv": { body: `<feed>${ytEntry("vid1", "First video")}</feed>` },
  "https://public.api.bsky.app/": {
    body: {
      feed: [
        { post: { uri: "at://did:plc:x/app.bsky.feed.post/abc", author: { handle: "me.bsky.social", displayName: "Me" }, record: { text: "Hello sky", createdAt: "2026-11-05T10:00:00Z" }, embed: { images: [{ fullsize: "https://cdn.bsky/1.jpg" }] } } },
        { reason: { $type: "repost" }, post: { uri: "at://did:plc:y/app.bsky.feed.post/zzz", author: { handle: "other" }, record: { text: "not mine" } } },
      ],
    },
  },
  "https://id.twitch.tv/oauth2/token": { body: { access_token: "tok", expires_in: 3600 } },
  "https://api.twitch.tv/helix/streams": { body: { data: [] } },
  "https://id.kick.com/oauth/token": { body: { access_token: "ktok", expires_in: 3600 } },
  "https://api.kick.com/public/v1/channels": { body: { data: [{ slug: "kicker", stream: { is_live: false } }] } },
  "https://api.steampowered.com/": { body: { appnews: { newsitems: [{ gid: "1", title: "Patch 1", url: "x", author: "dev", contents: "[b]Fixed[/b] things", date: 1793700000 }] } } },
  "https://blog.example/feed": { body: rss(2, 1) },
  "https://broken.example/": { status: 500, body: "oops" },
};
const write = () => fs.writeFileSync(answersFile, JSON.stringify(answers));
write();

const n = (block, values = {}) => ({ block: block + "@1", at: [0, 0], values });
const nodes = {
  yt: n("core.on_new_post", { source: "youtube", channel: "@chan", noShorts: true }),
  ytLog: n("core.log", { message: "youtube: ${title} | ${url} | ${image}", level: "info" }),
  sky: n("core.on_new_post", { source: "bluesky", channel: "me.bsky.social", announceOld: true }),
  skyLog: n("core.log", { message: "bluesky: ${title} | ${author} | ${url} | ${image}", level: "info" }),
  tw: n("core.on_new_post", { source: "twitch", channel: "https://twitch.tv/Streamer" }),
  twLog: n("core.log", { message: "twitch: ${title} | ${post.game} | ${post.viewers} | ${url}", level: "info" }),
  kick: n("core.on_new_post", { source: "kick", channel: "kick.com/Kicker" }),
  kickLog: n("core.log", { message: "kick: ${title} | ${post.game} | ${url} | ${image}", level: "info" }),
  steam: n("core.on_new_post", { source: "steam", channel: "https://store.steampowered.com/app/440/", announceOld: true }),
  steamLog: n("core.log", { message: "steam: ${title} | ${text} | ${url}", level: "info" }),
  blog: n("core.on_new_post", { source: "rss", url: "https://blog.example/feed" }),
  blogLog: n("core.log", { message: "blog: ${title} | ${text} | ${date.toISOString()}", level: "info" }),
  some: n("core.on_new_post", { source: "rss", url: "https://blog.example/feed", announceOld: true, contains: "post 1" }),
  someLog: n("core.log", { message: "filtered: ${title}", level: "info" }),
  bad: n("core.on_new_post", { source: "rss", url: "https://broken.example/feed" }),
};
const links = ["yt.next > ytLog.in", "sky.next > skyLog.in", "tw.next > twLog.in", "kick.next > kickLog.in", "steam.next > steamLog.in", "blog.next > blogLog.in", "some.next > someLog.in"];
const bundle = {
  project: { name: "Alerts", mode: "test", settings: { twitchClientId: "abc123", kickClientId: "kid" }, data: {} },
  blocks,
  libs: [],
  workspaces: { main: { name: "Main", nodes, links } },
};
const bundleFile = path.join(dir, "bundle.json");
fs.writeFileSync(bundleFile, JSON.stringify(bundle));

/** Runs the bot until `done(logs)` (or 20 s) and stops it. */
async function runBot(done) {
  const args = ["-r", path.join(__dirname, "fixtures", "mock-fetch.js"), path.join(__dirname, "..", "circuit-runtime.js"), "--bundle", bundleFile, "--deps", dir, "--data", path.join(dir, "data"), "--debug", "--stop-on-input"];
  const child = spawn(process.execPath, args, { stdio: ["pipe", "pipe", "pipe"], env: { ...process.env, XC_MOCK_FETCH: answersFile, XC_SECRET_TWITCHCLIENTSECRET: "shh", XC_SECRET_KICKCLIENTSECRET: "kshh" } });
  const seen = [];
  let buf = "";
  let err = "";
  child.stdout.on("data", (d) => {
    buf += d;
    for (const line of buf.split("\n").slice(0, -1)) {
      if (line.startsWith("@@xc ")) {
        const e = JSON.parse(line.slice(5));
        if (e.t === "log") seen.push(`${e.level}: ${e.text}`);
      }
    }
    buf = buf.slice(buf.lastIndexOf("\n") + 1);
  });
  child.stderr.on("data", (d) => (err += d));
  const exited = new Promise((r) => child.on("exit", r));
  const started = Date.now();
  while (!done(seen) && Date.now() - started < 20000) await new Promise((r) => setTimeout(r, 100));
  child.stdin.write("stop\n");
  await exited;
  if (err.trim()) seen.push(`stderr: ${err.trim()}`);
  return seen;
}

const asked = () => fs.readFileSync(`${answersFile}.log`, "utf8");
// Every alert checks once, 1 to 5 s after the start: all have been asked when the broken one has warned.
const checked = (seen) => seen.some((s) => s.includes("broken.example")) && asked().includes("blog.example") && asked().includes("helix/streams") && asked().includes("api.kick.com") && asked().includes("feeds/videos.xml") && asked().includes("steampowered") && asked().includes("bsky");

(async () => {
  // First run: what's there is only remembered, except where announceOld is on.
  const first = await runBot((s) => checked(s) && s.some((x) => x.includes("steam:")) && s.some((x) => x.includes("bluesky:")) && s.filter((x) => x.includes("filtered:")).length >= 1);
  const text = first.join("\n");
  assert.ok(first.some((s) => s.endsWith("bluesky: Hello sky | Me | https://bsky.app/profile/me.bsky.social/post/abc | https://cdn.bsky/1.jpg")), text);
  assert.ok(!text.includes("not mine"), "reposts aren't announced");
  assert.ok(first.some((s) => s.endsWith("steam: Patch 1 | Fixed things | https://store.steampowered.com/news/app/440/view/1")), text);
  assert.deepStrictEqual(first.filter((s) => s.includes("filtered:")), ["info: filtered: Post 1"], "contains: only Post 1");
  assert.ok(!text.includes("youtube:") && !text.includes("blog:") && !text.includes("twitch:") && !text.includes("kick:"), `nothing old announced without announceOld:\n${text}`);
  assert.strictEqual(first.filter((s) => s.includes("broken.example")).length, 1, "a failing source is reported once");
  assert.ok(first.some((s) => s.startsWith("warn:") && s.includes("RSS or Atom feed https://broken.example/feed") && s.includes("answered 500")), text);
  assert.ok(asked().includes("POST https://id.twitch.tv/oauth2/token") && asked().includes("user_login=streamer"), "Twitch: a token, then the stream by its name");

  // While the bot is off: a new blog post, a new video and a Short, the streamer goes live.
  answers["https://blog.example/feed"] = { body: rss(3, 2, 1) };
  answers["https://www.youtube.com/feeds/videos.xml?channel_id=UCabcdefghijklmnopqrstuv"] = { body: `<feed>${ytEntry("short", "A Short", true)}${ytEntry("vid22", "Second video")}${ytEntry("vid1", "First video")}</feed>` };
  answers["https://api.twitch.tv/helix/streams"] = {
    body: { data: [{ id: "s1", user_login: "streamer", user_name: "Streamer", title: "Live now", game_name: "Chess", viewer_count: 42, started_at: "2026-11-06T10:00:00Z", thumbnail_url: "https://t/{width}x{height}.jpg" }] },
  };
  answers["https://api.kick.com/public/v1/channels"] = {
    body: { data: [{ slug: "kicker", stream_title: "Kick live", category: { name: "Just Chatting" }, stream: { is_live: true, start_time: "2026-11-06T09:00:00Z", viewer_count: 7, thumbnail: "https://k/t.jpg" } }] },
  };
  write();
  fs.writeFileSync(`${answersFile}.log`, "");
  const second = await runBot((s) => checked(s) && s.some((x) => x.includes("blog:")) && s.some((x) => x.includes("youtube:")) && s.some((x) => x.includes("twitch:")) && s.some((x) => x.includes("kick:")));
  const text2 = second.join("\n");
  assert.deepStrictEqual(second.filter((s) => s.includes("blog:")), ["info: blog: Post 3 | Text 3 & more | 2026-11-03T10:00:00.000Z"], `only the new post, once:\n${text2}`);
  assert.deepStrictEqual(second.filter((s) => s.includes("youtube:")), ["info: youtube: Second video | https://www.youtube.com/watch?v=vid22 | https://i.ytimg.com/vi/vid22/hq.jpg"], `the new video, not the Short:\n${text2}`);
  assert.deepStrictEqual(second.filter((s) => s.includes("twitch:")), ["info: twitch: Live now | Chess | 42 | https://www.twitch.tv/streamer"], text2);
  assert.deepStrictEqual(second.filter((s) => s.includes("kick:")), ["info: kick: Kick live | Just Chatting | https://kick.com/kicker | https://k/t.jpg"], text2);
  assert.ok(asked().includes("POST https://id.kick.com/oauth/token") && asked().includes("slug=kicker"));
  assert.ok(!text2.includes("bluesky:") && !text2.includes("steam:"), "seen posts aren't announced again after a restart");
  assert.ok(!text2.includes("filtered: Post 1"), "nor the filtered one");
  fs.rmSync(dir, { recursive: true, force: true });
  console.log("alerts test passed");
  process.exit(0);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
