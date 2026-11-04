# Translate the app

XC Circuit's interface can be translated into any language. A language is a file in a plugin; you write it, save it, and the interface switches right away. In this tutorial you start a translation, fill in a few texts and see them in the app.

Blocks are not translated: their names, descriptions and fields stay in English, so that a project looks the same for everyone and help found online fits.

## 1. Start a translation

1. Open **Settings** (**Ctrl+,**) › **Appearance**.
2. Choose **Create a translation…**, and give the language's code (for example `fr`), its own name (*Français*) and its English name (*French*).

XC Circuit writes a plugin into your plugins folder with a `locales/fr.toml` that lists every text of the interface, and opens it.

## 2. Fill in texts

The file has a `[meta]` table and the texts under `[strings]`, by their English text:

```toml
[meta]
code = "fr"
name = "Français"
english_name = "French"

[strings]
"Save and close" = "Enregistrer et fermer"
"{count} to fix" = "{count} à corriger"
"Run" = ""
```

- Keep `{placeholders}` such as `{count}` as they are; they're filled in by the app.
- Empty entries show in English, so you can translate a little at a time.

Save the file. Pick the language in Settings › Appearance, and what you translated shows straight away. Every later save shows up too.

## 3. Share it

The translation is a plugin folder: zip it and share it, or propose it for the app itself as a pull request on [GitHub](https://github.com/XCraftTM/XC-Circuit) (German lives in `plugins/xc.core/locales/de.toml`).

Build modes have texts of their own (their categories and settings); a translation can add those as well. See [Translations](/api/locales).
