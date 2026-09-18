# Tools — Plot

Scientific/terminal plotting rendered off-screen to a text canvas via
`ratatui-plt`. Every tool returns the canvas as plain text and mirrors it into
output metadata (`plot` plain canvas, `plot_ansi` ANSI-coloured canvas); the
TUI renders the coloured canvas inline in the message window. Canvas is
clamped to 220x80 cells. All tools are read-only and always visible.

| Tool | Description |
|------|-------------|
| `plot_line` | XY line chart from one or more named series of `[x, y]` points. |
| `plot_scatter` | XY scatter with dot markers. |
| `plot_bar` | Bar chart from categories + datasets; `stacked` and `horizontal` modes. |
| `plot_histogram` | Histogram over a sample array; `count`/`density`/`probability` normalisation. |
| `plot_pie` | Pie (or `donut`) chart from labelled slices; auto 8-colour palette cycling. |
| `plot_heatmap` | 2D grid heatmap; `viridis`, `plasma`, `inferno`, `magma`, `coolwarm` colormaps. |

Common optional arguments: `title`, `x_label`, `y_label`, `width` (default
80, max 220), `height` (default 20, max 80). Colours accept named colours
(`red`, `cyan`, `lightgreen`, ...) or `#rrggbb` hex strings. String-encoded
payloads are coerced automatically; malformed input returns an error output
rather than crashing.

---

## plot_line

Render an XY line chart.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `series` | object or array | yes | One or more `{name?, data: [[x,y],...], color?}` series | `[{"name":"p50","data":[[1,12],[2,11]],"color":"cyan"}]` |
| `title`, `x_label`, `y_label` | string | no | Chart labels | `"Latency"` |
| `width` / `height` | integer | no | Canvas cells (80x20 default; max 220x80) | `100` / `24` |
| `x_grid`, `y_grid`, `show_legend` | boolean | no | Gridlines and legend toggles | `true` |

**Example:**
```text
plot_line title="Latency" series=[{"name":"p50","data":[[1,12],[2,11],[3,9]],"color":"cyan"},{"name":"p99","data":[[1,40],[2,38],[3,30]],"color":"red"}]
```

---

## plot_scatter

XY scatter plot with dot markers. Same arguments as `plot_line`.

```text
plot_scatter series=[{"name":"samples","data":[[1.0,2.1],[1.5,2.9],[2.0,4.2]]}]
```

---

## plot_bar

Bar chart over category labels with one or more datasets.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `categories` | array | yes | Category labels | `["openalex","wikipedia"]` |
| `datasets` | object or array | yes | One or more `{name?, data: [...], color?}` datasets | `[{"name":"hits","data":[14,22]}]` |
| `horizontal` | boolean | no | Horizontal bars | `false` |
| `stacked` | boolean | no | Stack datasets | `false` |
| `title`, `x_label`, `y_label`, `width`, `height` | — | no | Common options | — |

**Example:**
```text
plot_bar categories=["openalex","wikipedia"] datasets=[{"name":"hits","data":[14,22]}]
```

---

## plot_histogram

Histogram over a numeric sample array.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `data` | array | yes | Sample values | `[3,5,5,7,9,9,9,11]` |
| `bins` | integer | no | Bin count | `20` |
| `range` | array | no | `[min, max]` clamp | `[0,10]` |
| `norm` | enum | no | `count`, `density`, or `probability` | `"count"` |
| `cumulative` | boolean | no | Cumulative bins | `false` |
| `color`, `title`, `x_label`, `y_label`, `width`, `height` | — | no | Common options | — |

---

## plot_pie

Pie (or donut) chart from labelled slices.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `slices` | array | yes | `{label, value, color?}` entries | `[{"label":"rust","value":71}]` |
| `donut` | boolean | no | Render as donut | `false` |
| `radius` | number | no | Donut hole ratio [0,1) | `0.35` |
| `percentages`, `labels` | boolean | no | Toggle percentage and label overlays | `true` |
| `title`, `width`, `height` | — | no | Common options | — |

---

## plot_heatmap

2D grid heatmap.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `grid` | object | yes | `{values: [[...]]}` with optional `x`/`y` axis value arrays | `{"values":[[0,1],[2,3]]}` |
| `colormap` | string | no | `viridis`, `plasma`, `inferno`, `magma`, `coolwarm` | `"viridis"` |
| `colorbar`, `show_values` | boolean | no | Colour scale bar and cell value overlays | `true` / `false` |
| `title`, `x_label`, `y_label`, `width`, `height` | — | no | Common options | — |

**Example:**
```text
plot_heatmap grid={"values":[[0,1],[2,3]]} colormap="viridis"
```
