# Communications How-To Manual

This guide is a practical, end-to-end manual for using the communications
capabilities in `ragent`: email (Gmail) and external messaging channels
(Telegram and Discord).

It covers:

- What each tool is for
- Configuration (the `gmail` and `channels` blocks in `ragent.json`)
- OAuth2 authentication for Gmail
- Channel setup for Telegram and Discord
- Secret indirection with the `env:` prefix
- Tool actions and parameters
- Sending messages from the TUI, CLI, and HTTP API
- Testing
- Troubleshooting

---

## 1) What the communications tools are for

ragent ships two LLM-callable tools for external communications:

| Tool | Purpose | Category |
|------|---------|----------|
| `gmail` | Search, read, draft, and send Gmail messages using the Gmail REST API | Email |
| `send_channel_message` | Deliver text messages to Telegram bots and Discord webhooks | Messaging |

Both tools require configuration in `ragent.json` before they can operate.
The `gmail` tool additionally requires a one-time OAuth2 authentication step.

Both tools are registered in the default tool registry and are available to
the LLM during agent sessions. They use the `network:send` permission
category.

---

## 2) Configuration overview

Configuration lives in `ragent.json` (or `ragent.jsonc`) in the `.ragent/`
directory, with fallback to `~/.config/ragent/config.json`. The
communications-related keys are:

```jsonc
{
  // Gmail OAuth2 client credentials
  "gmail": {
    "client_id": "1234567890.apps.googleusercontent.com",
    "client_secret": "env:GMAIL_CLIENT_SECRET"
  },

  // External messaging channels
  "channels": {
    "enabled": true,
    "telegram": {
      "bot_token": "env:TELEGRAM_BOT_TOKEN",
      "chat_id": "env:TELEGRAM_CHAT_ID"
    },
    "discord": {
      "webhook_url": "env:DISCORD_WEBHOOK_URL"
    }
  }
}
```

### Config merge behaviour

When multiple config files are layered (global + project-local), the
project-local file overlays the global one:

- `channels.enabled`: if either file sets `true`, the merged result is `true`.
- `channels.telegram` / `channels.discord`: the project-local value replaces
  the global one if present.
- `gmail.client_id` / `gmail.client_secret` / `gmail.base_url`: project-local
  values override global ones when set.

---

## 3) Secret indirection with `env:`

All credential fields in the `gmail` and `channels` config blocks support
the `env:VAR_NAME` prefix. When a value starts with `env:`, ragent reads the
actual secret from the named environment variable at use time rather than
storing it in the config file.

| Field | Env-var fallback (when config value is absent) |
|-------|------------------------------------------------|
| `gmail.client_id` | `GMAIL_CLIENT_ID` |
| `gmail.client_secret` | `GMAIL_CLIENT_SECRET` |
| `channels.telegram.bot_token` | _(no fallback — must be in config or `env:`)_ |
| `channels.telegram.chat_id` | _(no fallback — must be in config or `env:`)_ |
| `channels.discord.webhook_url` | _(no fallback — must be in config or `env:`)_ |

Example using `env:` indirection:

```jsonc
{
  "channels": {
    "enabled": true,
    "telegram": {
      "bot_token": "env:TELEGRAM_BOT_TOKEN",
      "chat_id": "env:TELEGRAM_CHAT_ID"
    }
  }
}
```

```bash
export TELEGRAM_BOT_TOKEN="123456:ABC-DEF..."
export TELEGRAM_CHAT_ID="-1001234567890"
```

---

## 4) Gmail setup

### 4.1 Create Google OAuth2 credentials

1. Go to the [Google Cloud Console](https://console.cloud.google.com/).
2. Create or select a project.
3. Navigate to **APIs & Services → Credentials**.
4. Click **Create Credentials → OAuth client ID**.
   - Application type: **Web application** (or **Desktop app**).
   - Add an authorised redirect URI if using the OAuth Playground flow:
     `https://developers.google.com/oauthplayground`.
5. Note the **Client ID** and **Client Secret**.

### 4.2 Enable the Gmail API

1. In the Google Cloud Console, navigate to **APIs & Services → Library**.
2. Search for **Gmail API** and click **Enable**.

### 4.3 Obtain OAuth2 tokens

ragent does not run an interactive OAuth flow. You obtain tokens externally
and store them via the `gmail` tool's `auth` action.

The easiest method is the [Google OAuth2 Playground](https://developers.google.com/oauthplayground):

1. Click the gear icon (top right) and check **Use your own OAuth credentials**.
2. Enter your Client ID and Client Secret.
3. In the left panel, enter the Gmail scope:
   `https://mail.google.com/`.
4. Click **Authorize APIs** and grant access.
5. Click **Exchange authorization code for tokens**.
6. Copy the **Refresh token** (and optionally the Access token).

### 4.4 Configure ragent.json

Add the `gmail` block to your `ragent.json`:

```jsonc
{
  "gmail": {
    "client_id": "1234567890.apps.googleusercontent.com",
    "client_secret": "env:GMAIL_CLIENT_SECRET"
  }
}
```

```bash
export GMAIL_CLIENT_SECRET="GOCSPX-..."
```

Alternatively, you can pass the client credentials at auth time instead of
putting them in the config file (see §4.5).

### 4.5 Store tokens via the auth action

Once you have the tokens, store them in ragent's encrypted credential store
by asking the agent to run the `gmail` tool with `action: "auth"`:

**Via the TUI / agent prompt:**

```
Use the gmail tool to store my credentials: action="auth",
refresh_token="<your-refresh-token>",
client_id="<your-client-id>",
client_secret="<your-client-secret>"
```

**Via the HTTP API:**

```bash
curl -s -X POST http://localhost:9100/agent/message \
  -H "Authorization: Bearer $RAGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Use the gmail tool with action=auth, refresh_token=\"<token>\", client_id=\"<id>\", client_secret=\"<secret>\""
  }'
```

The tokens are encrypted and stored in the ragent SQLite database
(`<data_dir>/ragent/ragent.db`) using the same v2 machine-local encryption
scheme as provider credentials. They never appear in `ragent.json`.

### 4.6 Check authentication status

```
Use the gmail tool with action="status"
```

Returns:

```
gmail: authenticated=true, access_token=true, refresh_token=true
```

### 4.7 Logout (clear stored tokens)

```
Use the gmail tool with action="logout"
```

This removes all stored Gmail credentials from the encrypted store.

---

## 5) Gmail tool reference

### 5.1 Actions

| Action | Required parameters | Optional parameters | Description |
|--------|---------------------|---------------------|-------------|
| `search` | `query` | `max_results` (default 10, max 100) | Search the inbox using Gmail search syntax |
| `read` | `id` | — | Read a single message by ID (full body, up to 4000 chars) |
| `draft` | `to` | `subject`, `body`, `cc`, `bcc` | Create a draft email |
| `send` | `to` | `subject`, `body`, `cc`, `bcc` | Send an email immediately |
| `auth` | `access_token` and/or `refresh_token` | `client_id`, `client_secret` | Store OAuth2 credentials |
| `status` | — | — | Check if authenticated |
| `logout` | — | — | Clear stored credentials |

### 5.2 Search examples

Gmail search uses standard Gmail query syntax:

| Query | Matches |
|-------|--------|
| `from:ci@example.com is:unread` | Unread messages from a specific sender |
| `subject:"build failed"` | Messages with a subject containing "build failed" |
| `from:notifications@github.com newer_than:1d` | GitHub notifications from the last day |
| `has:attachment` | Messages with attachments |

### 5.3 Send example

```
Use the gmail tool to send an email:
  action="send",
  to="team@example.com",
  subject="Build succeeded",
  body="All tests passed in 42s.",
  cc="lead@example.com"
```

The tool constructs an RFC 2822 raw message (base64url-encoded) and sends it
via the Gmail `messages.send` endpoint. Drafts go to the `drafts` endpoint.

### 5.4 Automatic token refresh

When the stored access token is missing or expired, the tool automatically
refreshes it using the stored refresh token — provided `client_id` and
`client_secret` are available (from config, auth-time storage, or the
`GMAIL_CLIENT_ID` / `GMAIL_CLIENT_SECRET` environment variables).

If refresh fails, the error message tells you exactly what is missing.

---

## 6) Messaging channels setup

### 6.1 Telegram bot setup

1. Create a bot via [@BotFather](https://t.me/BotFather):
   ```
   /newbot
   ```
   Follow the prompts. You will receive a **bot token** (e.g.
   `123456789:ABCdefGhI...`).

2. Get your **chat ID**:
   - For a private chat: send any message to your bot, then visit
     `https://api.telegram.org/bot<TOKEN>/getUpdates` and look for
     `"chat":{"id":...}`.
   - For a group: add the bot to the group, send a message, and check
     `getUpdates`. Group chat IDs are negative numbers (e.g.
     `-1001234567890`).

3. Configure `ragent.json`:

   ```jsonc
   {
     "channels": {
       "enabled": true,
       "telegram": {
         "bot_token": "env:TELEGRAM_BOT_TOKEN",
         "chat_id": "env:TELEGRAM_CHAT_ID"
       }
     }
   }
   ```

4. Export the environment variables:

   ```bash
   export TELEGRAM_BOT_TOKEN="123456789:ABCdefGhI..."
   export TELEGRAM_CHAT_ID="-1001234567890"
   ```

### 6.2 Discord webhook setup

1. In your Discord server, go to the channel settings for the target
   channel.
2. Navigate to **Integrations → Webhooks → Create Webhook**.
3. Name the webhook (e.g. "ragent-notifications") and copy the **Webhook URL**
   (e.g. `https://discord.com/api/webhooks/123456789/abcdef...`).

4. Configure `ragent.json`:

   ```jsonc
   {
     "channels": {
       "enabled": true,
       "discord": {
         "webhook_url": "env:DISCORD_WEBHOOK_URL"
       }
     }
   }
   ```

5. Export the environment variable:

   ```bash
   export DISCORD_WEBHOOK_URL="https://discord.com/api/webhooks/..."
   ```

### 6.3 Combined Telegram + Discord configuration

You can configure both channels in the same `ragent.json`:

```jsonc
{
  "channels": {
    "enabled": true,
    "telegram": {
      "bot_token": "env:TELEGRAM_BOT_TOKEN",
      "chat_id": "env:TELEGRAM_CHAT_ID"
    },
    "discord": {
      "webhook_url": "env:DISCORD_WEBHOOK_URL"
    }
  }
}
```

When `channel: "all"` is specified, messages are delivered to both.

---

## 7) send_channel_message tool reference

### 7.1 Actions

| Action | Required parameters | Optional parameters | Description |
|--------|---------------------|---------------------|-------------|
| `send` | `message` | `channel` | Deliver a message (max 4096 bytes) |
| `list` | — | — | List configured channels and their readiness |
| `status` | — | — | Detailed readiness report (booleans, no secrets) |

If `action` is omitted, it defaults to `send`.

### 7.2 Channel targeting

| `channel` value | Behaviour |
|-----------------|----------|
| _(omitted)_ / `"auto"` | Sends to the first fully-configured channel (Telegram preferred if present) |
| `"telegram"` | Sends only to Telegram (fails if not configured) |
| `"discord"` | Sends only to Discord (fails if not configured) |
| `"all"` | Sends to every configured channel |

### 7.3 Send example

```
Use the send_channel_message tool to send:
  action="send",
  message="Build completed: all 247 tests passed in 38s.",
  channel="all"
```

### 7.4 Check channel status

```
Use the send_channel_message tool with action="status"
```

Returns a payload like:

```json
{
  "enabled": true,
  "configured": true,
  "channels": [
    {
      "kind": "telegram",
      "configured": true,
      "has_bot_token": true,
      "has_chat_id": true
    },
    {
      "kind": "discord",
      "configured": true,
      "has_webhook_url": true
    }
  ]
}
```

No secret material is ever included in the status response.

### 7.5 List configured channels

```
Use the send_channel_message tool with action="list"
```

Returns a one-line summary:

```
Configured channels: telegram (configured=true), discord (configured=true)
```

---

## 8) Using communications from the HTTP API

Both tools are available through the HTTP server's agent message endpoint.
Send a natural-language instruction and the LLM will invoke the appropriate
tool.

### 8.1 Send a Gmail message

```bash
curl -s -X POST http://localhost:9100/agent/message \
  -H "Authorization: Bearer $RAGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Use the gmail tool to send an email to ops@example.com with subject \"Deploy OK\" and body \"Production deploy completed.\""
  }'
```

### 8.2 Send a channel message

```bash
curl -s -X POST http://localhost:9100/agent/message \
  -H "Authorization: Bearer $RAGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Use send_channel_message to send \"Tests passed\" to all channels."
  }'
```

### 8.3 Search Gmail

```bash
curl -s -X POST http://localhost:9100/agent/message \
  -H "Authorization: Bearer $RAGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Use the gmail tool to search for unread emails from ci@example.com"
  }'
```

---

## 9) Testing

### 9.1 Running the existing test suites

The `gmail` and `send_channel_message` tools have comprehensive integration
tests that use mocked HTTP servers (axum) — no real API keys are needed.

**Gmail tests:**

```bash
cargo test -p ragent-tools-extended --test test_gmail
```

Covers: tool identity, parameters schema, config parsing, encrypted token
round-trip, auth/status/logout, mocked search/read/draft/send, refresh-token
exchange, RFC 2822 message construction, and graceful degradation when
unauthenticated.

**Channel messaging tests:**

```bash
cargo test -p ragent-tools-extended --test test_channels
```

Covers: tool identity, parameters schema, config parsing, `env:` secret
indirection, graceful degradation without config / when disabled, mocked
Telegram Bot API delivery, and mocked Discord webhook delivery.

**TUI rendering tests:**

```bash
cargo test -p ragent-tui --test test_message_widget_tests -- gmail
cargo test -p ragent-tui --test test_message_widget_tests -- send_channel_message
```

### 9.2 Manual testing with real credentials

#### Gmail

1. Complete the OAuth2 setup (§4.1–§4.5).
2. Verify status:

   ```
   Use the gmail tool with action="status"
   ```

3. Search your inbox:

   ```
   Use the gmail tool to search for "is:unread" with max_results=5
   ```

4. Send a test email to yourself:

   ```
   Use the gmail tool to send an email to myaddress@gmail.com,
   subject="ragent test", body="This is a test from ragent."
   ```

#### Telegram

1. Set up the bot (§6.1).
2. Check status:

   ```
   Use the send_channel_message tool with action="status"
   ```

3. Send a test message:

   ```
   Use the send_channel_message tool to send "Hello from ragent!" to telegram
   ```

#### Discord

1. Set up the webhook (§6.2).
2. Check status:

   ```
   Use the send_channel_message tool with action="status"
   ```

3. Send a test message:

   ```
   Use the send_channel_message tool to send "Hello from ragent!" to discord
   ```

---

## 10) Troubleshooting

### Gmail: "No Gmail access token stored"

You have not completed the OAuth2 auth step. Run the `auth` action with a
refresh token (and client_id/client_secret) as described in §4.5.

### Gmail: "Gmail refresh requires an OAuth client id"

The `client_id` is missing. Provide it via one of:
- `gmail.client_id` in `ragent.json`
- `client_id` parameter in the `auth` action
- `GMAIL_CLIENT_ID` environment variable

### Gmail: "Gmail refresh requires an OAuth client secret"

The `client_secret` is missing. Provide it via one of:
- `gmail.client_secret` in `ragent.json`
- `client_secret` parameter in the `auth` action
- `GMAIL_CLIENT_SECRET` environment variable

### Channels: "Channel messaging is disabled"

The `channels.enabled` flag is `false` (the default). Set it to `true` in
`ragent.json`:

```jsonc
{
  "channels": { "enabled": true }
}
```

### Channels: "No channels configured"

The `channels` block is missing from `ragent.json`, or neither Telegram nor
Discord is configured. Add at least one channel (§6.1 or §6.2).

### Channels: "Telegram channel is not fully configured"

Both `bot_token` and `chat_id` are required. If you used `env:` indirection,
make sure the environment variables are set and non-empty.

### Channels: "Discord channel is not fully configured"

The `webhook_url` is required. If you used `env:` indirection, make sure the
environment variable is set and non-empty.

### Channels: "Telegram send failed (HTTP 401)"

The bot token is invalid or has been revoked. Re-create the token via
BotFather and update your configuration.

### Channels: "Discord webhook send failed (HTTP 404)"

The webhook URL is invalid or the webhook has been deleted. Re-create the
webhook in Discord and update your configuration.

### Channels: "Message too long: N bytes (max 4096)"

The `send_channel_message` tool enforces a 4096-byte limit. Split your
message or send a shorter summary.

### Config not being picked up

- Ensure `ragent.json` is in the `.ragent/` directory (project-local) or
  `~/.config/ragent/config.json` (global).
- Use `/config show` in the TUI to inspect the resolved configuration.
- Remember that project-local config overlays global config.

---

## 11) Security notes

- **Gmail OAuth2 tokens** are stored encrypted in the ragent SQLite database
  using the same v2 machine-local encryption scheme as provider API keys.
  They never appear in `ragent.json` or log files.
- **Channel credentials** (bot tokens, webhook URLs) are resolved at use time
  when using the `env:` prefix, so they do not need to live in config files.
- The `send_channel_message` **status** action never includes secret
  material — only boolean flags indicating whether each channel is fully
  configured.
- Both tools use the `network:send` permission category, so they are subject
  to the standard permission system (allow/deny/ask rules, YOLO mode).

---

## 12) Related documentation

- [Configuration schema](../../SPEC.md) — full `ragent.json` reference
- [Custom agents](../custom-agents.md) — agent presets that can use these tools
- [Teams how-to](./howto_teams.md) — team coordination (which can trigger
  channel notifications)