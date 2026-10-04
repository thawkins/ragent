# Web source

- URL: https://github.com/openai/codex/commit/10ac2781eb7d83d6900686138242dfde12451453
- Title: chore: add JSON schema policy fixture coverage (#24152) · openai/codex@10ac278
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T13:31:51.077270813+00:00
- Relevance: High - title + snippet match query


```text
In openai/codex commit `10ac2781eb7d83d6900686138242dfde12451453`, the diff adds `codex-utils-cargo-bin = { workspace = true }` to a Cargo manifest and adds JSON tool-schema fixtures for Google Calendar (`google_calendar_create_event`), Google Drive (`google_drive_copy_file`), Microsoft Outlook Email (`microsoft_outlook_email_send`), and Notion (`notion_fetch_page`). The fixtures define schemas for calendar events, Drive file copying, Outlook messages/attachments, and Notion page fetching, and specify `expected_dropped_fields` for Calendar (`/definitions/DateTime/properties/dateTime/format`) and Drive (`/properties/file/oneOf`, `/properties/metadata/allOf`), while Outlook and Notion list none.
```
