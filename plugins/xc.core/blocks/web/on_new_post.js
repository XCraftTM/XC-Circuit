// On New Post: watches a source (a YouTube channel, a Twitch or Kick stream,
// a subreddit, a Bluesky or Mastodon account, GitHub releases, Steam news,
// any RSS or Atom feed) and runs once for each new post. What it has seen
// is kept with the stored data, so nothing is announced twice, not even
// after a restart.

const SOURCES = {
  youtube: "YouTube channel",
  twitch: "Twitch (goes live)",
  kick: "Kick (goes live)",
  reddit: "Reddit (subreddit)",
  bluesky: "Bluesky account",
  mastodon: "Mastodon account",
  github: "GitHub releases",
  steam: "Steam news",
  rss: "RSS or Atom feed",
};
const UA = "XC-Circuit-Bot/1.0 (+https://github.com/XCraftTM/XC-Circuit)";
/** How many ids of a source are remembered. */
const REMEMBER = 300;
/** At most this many posts per check; more at once are taken as seen. */
const PER_CHECK = 5;

// ───── Fetching ─────

async function get(url, { json = false, headers = {}, method = "GET", body } = {}) {
  const res = await fetch(url, { method, body, headers: { "User-Agent": UA, ...headers }, signal: AbortSignal.timeout(15000), redirect: "follow" });
  if (!res.ok) throw Object.assign(new Error(`${new URL(url).host} answered ${res.status}`), { status: res.status });
  return json ? res.json() : res.text();
}

// ───── Feeds (RSS and Atom), without a package ─────

function decode(s) {
  return String(s ?? "")
    .replace(/<!\[CDATA\[([\s\S]*?)\]\]>/g, "$1")
    .replace(/&#x([0-9a-f]+);/gi, (_, h) => String.fromCodePoint(parseInt(h, 16)))
    .replace(/&#(\d+);/g, (_, d) => String.fromCodePoint(Number(d)))
    .replace(/&quot;/g, '"')
    .replace(/&apos;|&#39;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&");
}

/** Text without HTML, in one paragraph per line, at most `max` characters. */
function plainText(html, max = 1000) {
  const text = decode(String(html ?? "").replace(/<!\[CDATA\[([\s\S]*?)\]\]>/g, "$1"))
    .replace(/<(br|\/p|\/div|\/li)\s*\/?>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .replace(/[ \t]+/g, " ")
    .replace(/\n\s*\n+/g, "\n\n")
    .trim();
  return text.length > max ? `${text.slice(0, max - 1).trimEnd()}…` : text;
}

const esc = (n) => n.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** The inside of the first `<name>` in `xml` (namespaced names such as media:description work). */
function tag(xml, name) {
  const m = new RegExp(`<${esc(name)}(?:\\s[^>]*)?>([\\s\\S]*?)</${esc(name)}>`, "i").exec(xml);
  return m ? decode(m[1]).trim() : "";
}

/** An attribute of the first `<name …>` whose attributes match `where`. */
function attr(xml, name, key, where) {
  for (const m of xml.matchAll(new RegExp(`<${esc(name)}\\s([^>]*?)/?>`, "gi"))) {
    if (where && !where.test(m[1])) continue;
    const a = new RegExp(`\\b${esc(key)}\\s*=\\s*("([^"]*)"|'([^']*)')`, "i").exec(m[1]);
    if (a) return decode(a[2] ?? a[3]);
  }
  return "";
}

/** The items of an RSS or Atom feed, newest first as feeds list them. */
function parseFeed(xml) {
  const out = [];
  for (const m of String(xml).matchAll(/<(item|entry)(?:\s[^>]*)?>([\s\S]*?)<\/\1>/gi)) {
    const x = m[2];
    const link = attr(x, "link", "href", /rel\s*=\s*["']alternate["']/) || attr(x, "link", "href") || tag(x, "link");
    const html = tag(x, "content:encoded") || tag(x, "content") || tag(x, "description") || tag(x, "summary") || tag(x, "media:description");
    const image =
      attr(x, "media:thumbnail", "url") ||
      attr(x, "media:content", "url", /medium\s*=\s*["']image|type\s*=\s*["']image/) ||
      attr(x, "enclosure", "url", /type\s*=\s*["']image/) ||
      (/<img[^>]+src\s*=\s*["']([^"']+)/i.exec(decode(html)) || [])[1] ||
      "";
    const date = tag(x, "pubDate") || tag(x, "published") || tag(x, "updated") || tag(x, "dc:date");
    const text = plainText(html);
    out.push({
      id: tag(x, "yt:videoId") || tag(x, "guid") || tag(x, "id") || link,
      // Posts without a title (Mastodon's) are titled by the start of their text.
      title: plainText(tag(x, "title"), 300) || plainText(text.split("\n")[0], 100),
      url: link,
      text,
      author: (tag(x, "name") || tag(x, "dc:creator") || tag(x, "author")).replace(/^\/u\//, "u/"),
      image,
      date: date ? new Date(date) : null,
    });
  }
  return out;
}

// ───── Sources ─────

/** A YouTube channel id from a handle (@name), a channel URL or the id itself. */
async function youtubeChannel(input, cache) {
  const s = String(input).trim();
  const id = /(UC[\w-]{22})/.exec(s);
  if (id) return id[1];
  if (cache.has(s)) return cache.get(s);
  const handle = s.replace(/^(https?:\/\/)?(www\.|m\.)?youtube\.com\//i, "").replace(/^@?/, "@").split(/[/?#]/)[0];
  // The cookie skips YouTube's consent page in Europe.
  const page = await get(`https://www.youtube.com/${handle}`, { headers: { Cookie: "SOCS=CAI; CONSENT=YES+1", "Accept-Language": "en" } });
  const found = /"(?:externalId|channelId)":"(UC[\w-]{22})"/.exec(page) || /youtube\.com\/channel\/(UC[\w-]{22})/.exec(page);
  if (!found) throw new Error(`no YouTube channel found for ${s}`);
  cache.set(s, found[1]);
  return found[1];
}

/** An app token for Twitch or Kick, kept until shortly before it ends. */
async function appToken(kind, ctx, cache) {
  const t = cache.get(`token:${kind}`);
  if (t && t.until > Date.now()) return t.token;
  const id = String(ctx.runtime.setting(`${kind}ClientId`) || "").trim();
  const secret = String(ctx.runtime.secret(`${kind}ClientSecret`) || "").trim();
  const name = kind === "twitch" ? "Twitch" : "Kick";
  if (!id || !secret) throw new Error(`${name} needs a client id and secret: Bot › Settings › New-post alerts`);
  const body = new URLSearchParams({ client_id: id, client_secret: secret, grant_type: "client_credentials" });
  const url = kind === "twitch" ? "https://id.twitch.tv/oauth2/token" : "https://id.kick.com/oauth/token";
  const r = await get(url, { json: true, method: "POST", body, headers: { "Content-Type": "application/x-www-form-urlencoded" } });
  cache.set(`token:${kind}`, { token: r.access_token, until: Date.now() + Math.max(60, (r.expires_in || 3600) - 300) * 1000 });
  return r.access_token;
}

const SOURCE_LIST = {
  async youtube(f, ctx, cache) {
    const id = await youtubeChannel(f.channel, cache);
    return parseFeed(await get(`https://www.youtube.com/feeds/videos.xml?channel_id=${id}`))
      .map((p) => ({ ...p, image: p.image || `https://i.ytimg.com/vi/${p.id}/hqdefault.jpg`, short: /\/shorts\//.test(p.url) }))
      .filter((p) => !(f.noShorts && p.short));
  },
  async twitch(f, ctx, cache) {
    const login = String(f.channel).trim().replace(/^(https?:\/\/)?(www\.)?twitch\.tv\//i, "").split(/[/?#]/)[0].toLowerCase();
    const token = await appToken("twitch", ctx, cache);
    const r = await get(`https://api.twitch.tv/helix/streams?user_login=${encodeURIComponent(login)}`, { json: true, headers: { "Client-Id": String(ctx.runtime.setting("twitchClientId")).trim(), Authorization: `Bearer ${token}` } });
    return (r.data || []).map((s) => ({
      id: s.id,
      title: s.title || `${s.user_name} is live`,
      url: `https://www.twitch.tv/${s.user_login || login}`,
      text: s.game_name ? `Playing ${s.game_name}` : "",
      author: s.user_name,
      image: String(s.thumbnail_url || "").replace("{width}", "1280").replace("{height}", "720") + `?t=${Date.now()}`,
      date: s.started_at ? new Date(s.started_at) : null,
      game: s.game_name || "",
      viewers: s.viewer_count ?? null,
    }));
  },
  async kick(f, ctx, cache) {
    const slug = String(f.channel).trim().replace(/^(https?:\/\/)?(www\.)?kick\.com\//i, "").split(/[/?#]/)[0].toLowerCase();
    const token = await appToken("kick", ctx, cache);
    const r = await get(`https://api.kick.com/public/v1/channels?slug=${encodeURIComponent(slug)}`, { json: true, headers: { Authorization: `Bearer ${token}` } });
    return (r.data || [])
      .filter((c) => c.stream && c.stream.is_live)
      .map((c) => ({
        id: `${c.slug || slug}:${c.stream.start_time || ""}`,
        title: c.stream_title || `${c.slug || slug} is live`,
        url: `https://kick.com/${c.slug || slug}`,
        text: c.category && c.category.name ? `Playing ${c.category.name}` : "",
        author: c.slug || slug,
        image: c.stream.thumbnail || "",
        date: c.stream.start_time ? new Date(c.stream.start_time) : null,
        game: (c.category && c.category.name) || "",
        viewers: c.stream.viewer_count ?? null,
      }));
  },
  async reddit(f) {
    const sub = String(f.channel).trim().replace(/^(https?:\/\/)?(www\.|old\.)?reddit\.com\//i, "").replace(/^\/?r\//, "").split(/[/?#]/)[0];
    try {
      const r = await get(`https://www.reddit.com/r/${encodeURIComponent(sub)}/new.json?limit=25&raw_json=1`, { json: true });
      return (r.data?.children || []).map(({ data: p }) => ({
        id: p.name || p.id,
        title: p.title,
        url: `https://www.reddit.com${p.permalink}`,
        text: plainText(p.selftext || ""),
        author: p.author ? `u/${p.author}` : "",
        image: p.preview?.images?.[0]?.source?.url || (/^https?:/.test(p.thumbnail || "") ? p.thumbnail : ""),
        date: p.created_utc ? new Date(p.created_utc * 1000) : null,
        nsfw: Boolean(p.over_18),
      }));
    } catch (e) {
      // Reddit sometimes refuses its JSON to bots; its feed is the same list.
      if (e.status !== 403 && e.status !== 429) throw e;
      return parseFeed(await get(`https://www.reddit.com/r/${encodeURIComponent(sub)}/new/.rss`));
    }
  },
  async bluesky(f) {
    const actor = String(f.channel).trim().replace(/^(https?:\/\/)?bsky\.app\/profile\//i, "").replace(/^@/, "").split(/[/?#]/)[0];
    const r = await get(`https://public.api.bsky.app/xrpc/app.bsky.feed.getAuthorFeed?actor=${encodeURIComponent(actor)}&filter=posts_no_replies&limit=20`, { json: true });
    return (r.feed || [])
      .filter((item) => !item.reason) // reposts aren't the account's own posts
      .map(({ post: p }) => ({
        id: p.uri,
        title: plainText(p.record?.text || "", 300),
        url: `https://bsky.app/profile/${p.author?.handle || actor}/post/${String(p.uri).split("/").pop()}`,
        text: p.record?.text || "",
        author: p.author?.displayName || p.author?.handle || actor,
        image: p.embed?.images?.[0]?.fullsize || p.embed?.images?.[0]?.thumb || p.embed?.external?.thumb || p.embed?.media?.images?.[0]?.fullsize || "",
        date: p.record?.createdAt ? new Date(p.record.createdAt) : null,
      }));
  },
  async mastodon(f) {
    // @user@instance (or a profile URL): the profile's own feed.
    const s = String(f.channel).trim();
    const url = /\/@/.test(s) ? (/^https?:\/\//i.test(s) ? s : `https://${s}`).replace(/\/?(\.rss)?$/, ".rss") : (() => {
      const [user, host] = s.replace(/^@/, "").split("@");
      if (!user || !host) throw new Error(`"${s}" isn't an account such as @name@mastodon.social`);
      return `https://${host}/@${user}.rss`;
    })();
    const account = /^https?:\/\/([^/]+)\/@([^/?#]+)/.exec(url);
    return parseFeed(await get(url)).map((p) => ({ ...p, author: p.author || (account ? `@${account[2].replace(/\.rss$/, "")}@${account[1]}` : s) }));
  },
  async github(f) {
    const repo = String(f.channel).trim().replace(/^(https?:\/\/)?(www\.)?github\.com\//i, "").split("/").slice(0, 2).join("/");
    if (!/^[\w.-]+\/[\w.-]+$/.test(repo)) throw new Error(`"${f.channel}" isn't a repository such as owner/name`);
    return parseFeed(await get(`https://github.com/${repo}/releases.atom`)).map((p) => ({ ...p, author: p.author || repo }));
  },
  async steam(f) {
    const app = (/(\d+)/.exec(String(f.channel)) || [])[1];
    if (!app) throw new Error(`"${f.channel}" isn't a Steam app id (the number in the store address)`);
    const r = await get(`https://api.steampowered.com/ISteamNews/GetNewsForApp/v2/?appid=${app}&count=10&feeds=steam_community_announcements`, { json: true });
    return (r.appnews?.newsitems || []).map((n) => {
      const text = String(n.contents || "").replace(/\[\/?[a-z0-9*]+[^\]]*\]/gi, "");
      const img = /\{STEAM_CLAN_IMAGE\}([^\s[\]"]+)/.exec(n.contents || "");
      return {
        id: String(n.gid),
        title: n.title,
        url: `https://store.steampowered.com/news/app/${app}/view/${n.gid}`,
        text: plainText(text),
        author: n.author || "",
        image: img ? `https://clan.akamai.steamstatic.com/images${img[1]}` : "",
        date: n.date ? new Date(n.date * 1000) : null,
      };
    });
  },
  async rss(f) {
    const url = String(f.url || "").trim();
    if (!/^https?:\/\//i.test(url)) throw new Error("Give the feed's address (https://…)");
    return parseFeed(await get(url));
  },
};

/** What a source is called in messages: "YouTube channel @name". */
const describe = (f) => `${SOURCES[f.source] || f.source} ${f.source === "rss" ? f.url : f.channel}`.trim();

module.exports = {
  id: "core.on_new_post",
  kind: "event",
  info: {
    name: "On New Post",
    description: "Runs for each new post of a YouTube channel, subreddit, Bluesky or Mastodon account, GitHub releases, Steam news or any RSS/Atom feed, and when a Twitch or Kick channel goes live. Nothing is announced twice, not even after a restart.",
    author: "XC Team",
    version: "1.0.0",
  },
  search: {
    category: "web",
    aliases: ["YouTube", "Twitch", "Kick", "Reddit", "Bluesky", "Mastodon", "RSS", "Feed", "Stream live", "New video", "Notifications", "Social alerts", "GitHub release", "Steam news"],
    tags: ["youtube", "twitch", "kick", "reddit", "bluesky", "mastodon", "rss", "atom", "feed", "live", "stream", "video", "alert", "notify", "github", "steam", "social"],
  },
  look: {
    icon: "send",
  },
  fields: {
    source: { type: "select", pin: false, default: "youtube", choices: SOURCES, description: "What to watch." },
    channel: {
      type: "text",
      pin: false,
      default: "",
      required: true,
      show: { source: Object.keys(SOURCES).filter((s) => s !== "rss") },
      placeholder: "@name, r/name, owner/repo …",
      description:
        "YouTube: @handle, channel link or id · Twitch/Kick: the channel name · Reddit: the subreddit · Bluesky: name.bsky.social · Mastodon: @name@server · GitHub: owner/repo · Steam: the app id (the number in the store link).",
    },
    url: { type: "text", pin: false, default: "", required: true, show: { source: ["rss"] }, label: "Feed address", placeholder: "https://…/feed.xml", description: "The address of an RSS or Atom feed. Many sites have one (news, blogs, podcasts)." },
    noShorts: { type: "boolean", pin: false, default: false, show: { source: ["youtube"] }, label: "Skip Shorts", description: "Only videos, not Shorts." },
    every: { type: "number", pin: false, default: 5, min: 1, max: 1440, label: "Check every (min)", description: "How often to look, in minutes." },
    contains: { type: "text", pin: false, default: "", advanced: true, label: "Only if it contains", description: "Only posts whose title or text contains this (upper/lower case doesn't matter). Empty: all." },
    announceOld: {
      type: "boolean",
      pin: false,
      default: false,
      advanced: true,
      label: "Announce what's there at first",
      description: "The first time, posts that are already there count as seen and aren't announced. On: the newest few are announced too.",
    },
  },
  outputs: {
    title: { type: "text", description: "The post's title (for streams, the stream's title)." },
    url: { type: "text", description: "Where it is." },
    text: { type: "text", description: "Its text, without formatting (shortened)." },
    author: { type: "text", description: "Who posted it." },
    image: { type: "text", description: "A picture of it (thumbnail), or empty." },
    date: { type: "date?", description: "When it was posted (or the stream started)." },
    post: { type: "object", description: "Everything known about it, with what the source has besides: game and viewers for streams, nsfw for Reddit, short for YouTube." },
  },

  listen(ctx, emit) {
    const f = ctx.fields;
    const cache = new Map();
    const store = ctx.store.ns("xc.alerts");
    const key = `${ctx.id}:${f.source}:${f.source === "rss" ? f.url : f.channel}`;
    const filter = String(f.contains || "").trim().toLowerCase();
    let failing = false;

    const check = async () => {
      let posts;
      try {
        if (!SOURCE_LIST[f.source]) throw new Error(`unknown source "${f.source}"`);
        posts = (await SOURCE_LIST[f.source](f, ctx, cache)).filter((p) => p && p.id);
        // Newest first, whatever order the source gives.
        if (posts.every((p) => p.date instanceof Date && !Number.isNaN(p.date.getTime()))) posts.sort((a, b) => b.date - a.date);
      } catch (e) {
        // Said once, not every few minutes, until it works again.
        if (!failing) ctx.log.warn(`On New Post (${describe(f)}): ${e.message}. It keeps trying.`);
        failing = true;
        return;
      }
      if (failing) ctx.log.info(`On New Post (${describe(f)}) works again.`);
      failing = false;
      const saved = (await store.get(key)) || { seen: [], primed: false };
      const seen = new Set(saved.seen);
      let fresh = posts.filter((p) => !seen.has(String(p.id)));
      if (!saved.primed && !f.announceOld) fresh = [];
      // Oldest first, a few at a time.
      const shown = fresh.slice(0, PER_CHECK).reverse();
      const remember = [...posts.map((p) => String(p.id)), ...saved.seen.filter((id) => !posts.some((p) => String(p.id) === id))].slice(0, REMEMBER);
      await store.set(key, { seen: remember, primed: true });
      for (const p of shown) {
        if (filter && !`${p.title}\n${p.text}`.toLowerCase().includes(filter)) continue;
        emit({ title: p.title || "", url: p.url || "", text: p.text || "", author: p.author || "", image: p.image || "", date: p.date || null, post: { source: f.source, ...p } });
      }
    };

    ctx.runtime.onStart(() => {
      // A few seconds apart, so many alerts don't all ask at once.
      setTimeout(check, 1000 + Math.random() * 4000);
      setInterval(check, Math.max(1, Number(f.every) || 5) * 60 * 1000);
    });
  },

  // For tests and checks against the real sites.
  sources: SOURCE_LIST,
  parseFeed,
};
