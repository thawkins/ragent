# Tools — Communications

Gmail access and outbound notifications to Telegram/Discord channels. See
`docs/howtos/communications.md` for the full OAuth2 and channel setup guide.

| Tool | Description |
|------|-------------|
| `gmail` | Search, read, draft, and send Gmail messages. |
| `send_channel_message` | Post to Telegram or Discord channels. |

---

## gmail

Search, read, draft, and send Gmail messages via the Gmail REST API (requires
prior OAuth2 authentication through the `auth` action).

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `action` | enum | yes | `search`, `read`, `draft`, `send`, `auth`, `status`, `logout` | `"search"` |
| `query` | string | yes for `search` | Gmail search query | `"from:ci@example.com is:unread"` |
| `id` | string | yes for `read` | Message id | — |
| `max_results` | integer | no | `search` result cap (max 100) | `10` |
| `to` | string | yes for `draft`/`send` | Recipient address | `"dev@example.com"` |
| `subject` | string | yes for `draft`/`send` | Subject line | `"Build report"` |
| `body` | string | yes for `draft`/`send` | Plain-text body | — |
| `cc` / `bcc` | string | no | Cc/Bcc headers | — |
| `access_token` | string | for `auth` | OAuth2 access token to store | — |
| `refresh_token` | string | for `auth` | Refresh token (enables automatic access-token refresh) | — |
| `client_id` / `client_secret` | string | for `auth` | OAuth2 client credentials for refresh exchange | — |

**Example:**
```text
gmail action="search" query="from:ci@example.com is:unread"
gmail action="send" to="dev@example.com" subject="Build passed" body="All tests green"
```

---

## send_channel_message

Send a message to a configured Telegram bot or Discord webhook. Channels are
configured in `ragent.json` under the `channels` block; set
`channels.enabled=true` to allow delivery.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `action` | enum | no | `send` (default), `list`, `status` | `"send"` |
| `message` | string | yes for `send` | Message text (max 4096 bytes) | `"Build passed"` |
| `channel` | enum | no | `telegram`, `discord`, or `all` for every configured channel; omitted targets the first configured one | `"telegram"` |

**Example:**
```text
send_channel_message action="send" message="Build passed" channel="telegram"
```
