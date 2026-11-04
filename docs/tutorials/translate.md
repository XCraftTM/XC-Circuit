# Translate the app

Add a language for XC Circuit's menus and windows. (Blocks stay in English, so projects look the same for everyone.)

## 1. Start

**Settings** (gear at the bottom left, or **Ctrl+,**) › **Appearance**:

![Settings › Appearance](/screens/translate.png)

1. **Create a translation…**, then give the language's code (`fr`) and name (*Français*).

A file opens with every text of the app.

## 2. Translate

```toml
[strings]
"Save and close" = "Enregistrer et fermer"
"{count} to fix" = "{count} à corriger"
"Run" = ""
```

- Keep `{words in braces}` as they are.
- Empty lines stay English: translate a bit at a time.

Save, and pick your language under **Interface language**. Changes show right away.

## 3. Share

It's a plugin folder: zip it and share it (others install it with **Plugins › Install from Zip…**), or send it to us as a pull request on [GitHub](https://github.com/XCraftTM/XC-Circuit).

More: [Translations](/api/locales).
