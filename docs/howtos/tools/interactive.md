# Tools — Interactive

Ask the user questions, record reasoning notes, evaluate expressions, and
read environment variables.

| Tool | Description |
|------|-------------|
| `ask_user` | Ask the user a question (with optional choices). |
| `think` | Record a short reasoning note. |
| `calculator` | Evaluate a mathematical expression. |
| `get_env` | Read environment variable values. |

---

## ask_user

Ask the user a question and wait for the typed response. With `options` the
user picks from a multiple-choice dialog; otherwise they type free text.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `question` | string | yes | The prompt shown to the user | `"Which build profile?"` |
| `options` | array | no | Fixed choices; the returned value will be one of these strings | `["Debug","Release","Check only"]` |

**Example:**
```text
ask_user question="Which build profile?" options=["Debug","Release","Check only"]
```

---

## think

Record a short reasoning note in the session log without changing project
state (published as a reasoning event). Keep entries brief and focused on
the next action.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `thought` | string | yes | Short reasoning note |

**Example:** `think thought="Retrying the edit with the verbatim needle from line 42."`

---

## calculator

Evaluate a mathematical expression (Python arithmetic + the `math` module;
integer/float/complex). Runs in a short-lived `python3` process with a
10-second timeout.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `expression` | string | yes | Python-syntax expression | `"math.factorial(20)"`, `"2 ** 32"` |

**Example:** `calculator expression="math.sqrt(2 * math.pi)"`

---

## get_env

Read one or more environment variables. Names containing `KEY`, `SECRET`,
`TOKEN`, `PASSWORD` (or similar) are redacted to avoid leaking credentials.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | one of two | Single variable name |
| `names` | array | one of two | List of variable names |

**Example:** `get_env name="HOME"`
