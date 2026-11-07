import { defineConfig } from "vitepress";

const repo = "https://github.com/XCraftTM/XC-Circuit";
const base = process.env.DOCS_BASE ?? "/XC-Circuit/";
// Where the site lives, for link previews (Discord and others need full URLs).
const site = (process.env.DOCS_SITE ?? "https://xcrafttm.github.io") + base;

// The site and its docs: tutorials (learn by doing), explanations (how things
// work and why) and the API reference (every key and function).
export default defineConfig({
  lang: "en",
  title: "XC™ Circuit",
  description: "Build Discord and Fluxer bots by connecting blocks. Extend the editor with JavaScript plugins.",
  // GitHub Pages serves the site under /XC-Circuit/; DOCS_BASE changes that (for example "/" on a custom domain).
  base,
  cleanUrls: true,
  // Dark like the app; the switch in the top bar still offers light.
  appearance: "dark",
  lastUpdated: true,
  // Notes for the people building the app, not pages of the site.
  srcExclude: ["discord-blocks-plan.md"],
  head: [
    ["link", { rel: "icon", type: "image/svg+xml", href: `${base}icon.svg` }],
    ["meta", { name: "theme-color", content: "#00E5FF" }],
    // The picture shown when a link to the site is posted.
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:site_name", content: "XC™ Circuit" }],
    ["meta", { property: "og:image", content: `${site}social/github-preview.png` }],
    ["meta", { property: "og:image:width", content: "1280" }],
    ["meta", { property: "og:image:height", content: "640" }],
    ["meta", { name: "twitter:card", content: "summary_large_image" }],
  ],
  // Each page's own title and description in link previews.
  transformHead: ({ title, description }) => [
    ["meta", { property: "og:title", content: title }],
    ["meta", { property: "og:description", content: description }],
  ],
  themeConfig: {
    logo: "/icon.svg",
    siteTitle: "XC™ Circuit",
    nav: [
      { text: "Tutorials", link: "/tutorials/", activeMatch: "/tutorials/" },
      { text: "Explanations", link: "/explanations/", activeMatch: "/explanations/" },
      { text: "Plugins", link: "/plugins/", activeMatch: "/plugins/" },
      { text: "API", link: "/api/", activeMatch: "/api/" },
      { text: "Download", link: "/download" },
      {
        text: "More",
        items: [
          { text: "Changelog", link: "/changelog" },
          { text: "Known issues", link: "/known-issues" },
          { text: "Performance", link: "/performance" },
          { text: "How XC Circuit is made", link: "/made-with-ai" },
          { text: "Report a bug", link: "https://github.com/XCraftTM/XC-Circuit/issues/new?template=bug_report.yml" },
        ],
      },
    ],
    sidebar: {
      "/tutorials/": [
        {
          text: "Tutorials",
          items: [
            { text: "Overview", link: "/tutorials/" },
            { text: "Your first bot", link: "/tutorials/first-bot" },
            { text: "Values and texts", link: "/tutorials/values-and-texts" },
            { text: "Your first block", link: "/tutorials/first-block" },
            { text: "Your first editor plugin", link: "/tutorials/first-editor-plugin" },
            { text: "Translate the app", link: "/tutorials/translate" },
          ],
        },
      ],
      "/explanations/": [
        {
          text: "Explanations",
          items: [
            { text: "Overview", link: "/explanations/" },
            { text: "Events, actions and values", link: "/explanations/blocks-and-wires" },
            { text: "Texts and ${…}", link: "/explanations/expressions" },
            { text: "Projects and workspaces", link: "/explanations/projects" },
            { text: "Variables and stored data", link: "/explanations/data" },
            { text: "Databases", link: "/explanations/databases" },
            { text: "Pictures with Build Canvas", link: "/explanations/canvas" },
            { text: "New-post alerts", link: "/explanations/alerts" },
            { text: "Server tools", link: "/explanations/server-tools" },
            { text: "Captcha", link: "/explanations/captcha" },
            { text: "Transcripts", link: "/explanations/transcripts" },
            { text: "Try Zones and errors", link: "/explanations/errors" },
            { text: "Plugins", link: "/explanations/plugins" },
            { text: "Running and exporting", link: "/explanations/running" },
            { text: "AI agents (MCP)", link: "/explanations/ai-agents" },
            { text: "Tokens and safety", link: "/explanations/safety" },
            { text: "Everything the editor does", link: "/explanations/editor-reference" },
          ],
        },
      ],
      "/plugins/": [
        {
          text: "Plugin guide",
          items: [
            { text: "Make a plugin", link: "/plugins/" },
            { text: "Recipes", link: "/plugins/recipes" },
            { text: "Share and update", link: "/plugins/share" },
          ],
        },
        {
          text: "Tutorials",
          items: [
            { text: "Your first block", link: "/tutorials/first-block" },
            { text: "Your first editor plugin", link: "/tutorials/first-editor-plugin" },
          ],
        },
        { text: "Reference", items: [{ text: "Every key and function", link: "/api/" }] },
      ],
      "/api/": [
        { text: "Plugin guide", items: [{ text: "Make a plugin", link: "/plugins/" }, { text: "Recipes", link: "/plugins/recipes" }, { text: "Share and update", link: "/plugins/share" }] },
        { text: "Reference", items: [{ text: "What do you want to do?", link: "/api/" }] },
        {
          text: "Blocks",
          items: [
            { text: "Blocks", link: "/api/blocks" },
            { text: "Fields and types", link: "/api/fields" },
            { text: "Run context (ctx)", link: "/api/ctx" },
          ],
        },
        {
          text: "Editor plugins",
          items: [
            { text: "Editor plugins (xc)", link: "/api/editor" },
            { text: "Panels (ui)", link: "/api/panels" },
          ],
        },
        {
          text: "Plugin files",
          items: [
            { text: "plugin.toml", link: "/api/plugin-toml" },
            { text: "Build modes", link: "/api/build-modes" },
            { text: "Runtime libraries", link: "/api/runtime-libs" },
            { text: "Pin types", link: "/api/pin-types" },
            { text: "Themes", link: "/api/themes" },
            { text: "Translations", link: "/api/locales" },
            { text: "Templates", link: "/api/templates" },
          ],
        },
        { text: "Projects", items: [{ text: "Project format", link: "/api/project-format" }] },
      ],
    },
    socialLinks: [{ icon: "github", link: repo }],
    search: { provider: "local" },
    editLink: { pattern: `${repo}/edit/main/docs/:path`, text: "Suggest a change to this page" },
    outline: { level: [2, 3] },
    footer: {
      message: 'Source-available under the <a href="https://github.com/XCraftTM/XC-Circuit/blob/main/LICENSE">XC™ Circuit Licence</a>.',
      copyright: 'Copyright © 2026 <a href="https://github.com/XCraftTM">XCraftTM</a>',
    },
  },
});
