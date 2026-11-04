# Translations

::: info In short
A TOML file per language: the app's English texts and their translations.
:::

A translation is a TOML file per language in a plugin, listed in `contributes.locales`. It translates the interface by its English texts. Blocks are not translated. See the [tutorial](/tutorials/translate) for how to start one from the app.

```toml
[meta]
code = "de"
name = "Deutsch"           # the language's own name, shown in the picker
english_name = "German"

[strings]
"Save and close" = "Speichern und schließen"
"{count} to fix" = "{count} zu beheben"
"Run" = ""                 # empty: shown in English
```

## meta

| Key | Description |
| --- | --- |
| `code` | The language code: `de`, `fr`, `pt-BR`. |
| `name` | The language's name in itself. |
| `english_name` | Its name in English. |

## strings

Each key is an English text exactly as the app shows it; each value its translation.

- Keep `{placeholders}` such as `{count}` or `{name}`: the app fills them in. You may move them.
- Empty values fall back to English.
- Several plugins can add to the same language. The app's own texts are in Circuit Core's file; each build mode translates its own category names and settings in its plugin.

Settings › Appearance › *Create a translation…* writes a file with every text of the app to fill in. Files reload when saved.
