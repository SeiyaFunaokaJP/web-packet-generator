# Adding i18n strings

Status: draft

All user-facing text is stored in Fluent `.ftl` files under `locales/ja` and `locales/en`, for both server and web. Hard-coded strings are banned and a CI lint reports missing keys. Protocol names and field descriptions are localized in the same way. Adding a new language means adding a locale directory and translating the keys.
