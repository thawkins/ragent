//! HTML templates and CSS/JS for AIWiki web interface.

use crate::Aiwiki;
use std::path::Path;

/// CSS content for AIWiki styles.
pub const CSS_CONTENT: &str = r#"
:root {
    --bg-primary: #ffffff;
    --bg-secondary: #f5f5f5;
    --bg-tertiary: #e8e8e8;
    --text-primary: #1a1a1a;
    --text-secondary: #666666;
    --text-muted: #999999;
    --accent-primary: #2563eb;
    --accent-secondary: #3b82f6;
    --border-color: #e0e0e0;
    --success: #10b981;
    --warning: #f59e0b;
    --error: #ef4444;
    --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    --font-mono: 'SF Mono', Monaco, Consolas, monospace;
}

@media (prefers-color-scheme: dark) {
    :root {
        --bg-primary: #0d1117;
        --bg-secondary: #161b22;
        --bg-tertiary: #21262d;
        --text-primary: #c9d1d9;
        --text-secondary: #8b949e;
        --text-muted: #6e7681;
        --accent-primary: #58a6ff;
        --accent-secondary: #79c0ff;
        --border-color: #30363d;
    }
}

* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: var(--font-sans);
    background: var(--bg-primary);
    color: var(--text-primary);
    line-height: 1.6;
}

.navbar {
    background: var(--bg-secondary);
    border-bottom: 1px solid var(--border-color);
    padding: 1rem 2rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
}

.navbar-brand {
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--text-primary);
    text-decoration: none;
}

.navbar-brand:hover {
    color: var(--accent-primary);
}

.navbar-nav {
    display: flex;
    gap: 1.5rem;
    list-style: none;
}

.navbar-nav a {
    color: var(--text-secondary);
    text-decoration: none;
    font-weight: 500;
}

.navbar-nav a:hover {
    color: var(--accent-primary);
}

.main-container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem;
}

.page-header {
    margin-bottom: 2rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--border-color);
}

.page-title {
    font-size: 2rem;
    font-weight: 700;
    color: var(--text-primary);
    margin-bottom: 0.5rem;
}

.page-meta {
    color: var(--text-muted);
    font-size: 0.875rem;
}

.content {
    background: var(--bg-primary);
    padding: 2rem 0;
}

.content h1 {
    font-size: 2rem;
    margin: 1.5rem 0 1rem;
    padding-bottom: 0.5rem;
    border-bottom: 2px solid var(--border-color);
}

.content h2 {
    font-size: 1.5rem;
    margin: 1.5rem 0 0.75rem;
}

.content h3 {
    font-size: 1.25rem;
    margin: 1.25rem 0 0.5rem;
}

.content p {
    margin: 0.75rem 0;
}

.content a {
    color: var(--accent-primary);
    text-decoration: none;
}

.content a:hover {
    text-decoration: underline;
}

.content ul, .content ol {
    margin: 1rem 0;
    padding-left: 2rem;
}

.content li {
    margin: 0.25rem 0;
}

.content code {
    background: var(--bg-tertiary);
    padding: 0.2rem 0.4rem;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 0.9em;
}

.content pre {
    background: var(--bg-secondary);
    padding: 1rem;
    border-radius: 8px;
    overflow-x: auto;
    margin: 1rem 0;
}

.content pre code {
    background: none;
    padding: 0;
}

.content blockquote {
    border-left: 4px solid var(--accent-primary);
    padding-left: 1rem;
    margin: 1rem 0;
    color: var(--text-secondary);
}

.content table {
    width: 100%;
    border-collapse: collapse;
    margin: 1rem 0;
}

.content th, .content td {
    padding: 0.75rem;
    border: 1px solid var(--border-color);
    text-align: left;
}

.content th {
    background: var(--bg-secondary);
    font-weight: 600;
}

.content img {
    max-width: 100%;
    height: auto;
    border-radius: 8px;
}

.wiki-link {
    color: var(--accent-primary);
    font-weight: 500;
}

.wiki-link.missing {
    color: var(--error);
    opacity: 0.7;
}

.edit-button {
    background: var(--accent-primary);
    color: white;
    border: none;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.875rem;
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
}

.edit-button:hover {
    background: var(--accent-secondary);
}

.search-box {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 2rem;
}

.search-box input {
    flex: 1;
    padding: 0.75rem 1rem;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    background: var(--bg-primary);
    color: var(--text-primary);
    font-size: 1rem;
}

.search-box input:focus {
    outline: none;
    border-color: var(--accent-primary);
}

.search-box button {
    background: var(--accent-primary);
    color: white;
    border: none;
    padding: 0.75rem 1.5rem;
    border-radius: 6px;
    cursor: pointer;
    font-weight: 500;
}

.search-box button:hover {
    background: var(--accent-secondary);
}

.search-results {
    list-style: none;
    padding: 0;
}

.search-result {
    padding: 1rem 0;
    border-bottom: 1px solid var(--border-color);
}

.search-result:last-child {
    border-bottom: none;
}

.search-result-title {
    font-size: 1.125rem;
    font-weight: 600;
    margin-bottom: 0.25rem;
}

.search-result-title a {
    color: var(--accent-primary);
    text-decoration: none;
}

.search-result-title a:hover {
    text-decoration: underline;
}

.search-result-excerpt {
    color: var(--text-secondary);
    font-size: 0.9rem;
}

.search-result-meta {
    color: var(--text-muted);
    font-size: 0.8rem;
    margin-top: 0.25rem;
}

.editor-container {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}

.editor-textarea {
    width: 100%;
    min-height: 500px;
    padding: 1rem;
    border: 1px solid var(--border-color);
    border-radius: 8px;
    background: var(--bg-primary);
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 0.9rem;
    resize: vertical;
}

.editor-actions {
    display: flex;
    gap: 1rem;
}

.btn {
    padding: 0.75rem 1.5rem;
    border-radius: 6px;
    font-weight: 500;
    cursor: pointer;
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    border: none;
}

.btn-primary {
    background: var(--accent-primary);
    color: white;
}

.btn-primary:hover {
    background: var(--accent-secondary);
}

.btn-secondary {
    background: var(--bg-tertiary);
    color: var(--text-primary);
}

.btn-secondary:hover {
    background: var(--border-color);
}

.status-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 1.5rem;
    margin: 2rem 0;
}

.status-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1.5rem;
}

.status-card-title {
    font-size: 0.875rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 0.5rem;
}

.status-card-value {
    font-size: 2rem;
    font-weight: 700;
    color: var(--text-primary);
}

.status-card-subtitle {
    font-size: 0.875rem;
    color: var(--text-secondary);
    margin-top: 0.25rem;
}

.graph-container {
    width: 100%;
    height: 600px;
    border: 1px solid var(--border-color);
    border-radius: 8px;
    background: var(--bg-secondary);
    overflow: hidden;
}

.graph-container svg {
    cursor: grab;
}

.graph-container svg:active {
    cursor: grabbing;
}

.graph-container .node {
    cursor: pointer;
}

.graph-container .node:hover circle {
    stroke: var(--text-primary);
    stroke-width: 2;
}

.setup-container {
    max-width: 600px;
    margin: 5rem auto;
    text-align: center;
    padding: 2rem;
}

.setup-title {
    font-size: 2rem;
    margin-bottom: 1rem;
}

.setup-description {
    color: var(--text-secondary);
    margin-bottom: 2rem;
}

.setup-steps {
    text-align: left;
    background: var(--bg-secondary);
    border-radius: 8px;
    padding: 1.5rem;
    margin-bottom: 2rem;
}

.setup-steps ol {
    margin-left: 1.5rem;
}

.setup-steps li {
    margin: 0.75rem 0;
}

.error-container {
    max-width: 600px;
    margin: 5rem auto;
    text-align: center;
    padding: 2rem;
}

.error-title {
    color: var(--error);
    font-size: 1.5rem;
    margin-bottom: 1rem;
}

.error-message {
    color: var(--text-secondary);
}

.breadcrumbs {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
    font-size: 0.875rem;
}

.breadcrumbs a {
    color: var(--text-secondary);
    text-decoration: none;
}

.breadcrumbs a:hover {
    color: var(--accent-primary);
}

.breadcrumbs .separator {
    color: var(--text-muted);
}
"#;

/// JavaScript content for AIWiki interactivity.
pub const JS_CONTENT: &str = r#"
// AIWiki Web Interface JavaScript

document.addEventListener('DOMContentLoaded', function() {
    // Initialize syntax highlighting if available
    if (typeof hljs !== 'undefined') {
        hljs.highlightAll();
    }

    // Handle wiki link clicks
    document.querySelectorAll('.wiki-link').forEach(link => {
        link.addEventListener('click', function(e) {
            const href = this.getAttribute('href');
            if (href.startsWith('/aiwiki/page/')) {
                // Internal navigation - let it proceed
                return true;
            }
        });
    });

    // Search form handling
    const searchForm = document.getElementById('search-form');
    if (searchForm) {
        searchForm.addEventListener('submit', function(e) {
            const query = document.getElementById('search-input').value.trim();
            if (!query) {
                e.preventDefault();
            }
        });
    }

    // Graph visualization (if present)
    const graphContainer = document.getElementById('graph-viz');
    if (graphContainer) {
        initGraphVisualization(graphContainer);
    }
});

// Initialize D3.js graph visualization
function initGraphVisualization(container) {
    // Fetch graph data
    fetch('/aiwiki/api/graph')
        .then(response => response.json())
        .then(data => renderGraph(container, data))
        .catch(err => {
            container.innerHTML = '<div class="error-message">Failed to load graph: ' + err.message + '</div>';
        });
}

function renderGraph(container, data) {
    const width = container.clientWidth;
    const height = container.clientHeight;

    const svg = d3.select(container)
        .append('svg')
        .attr('width', width)
        .attr('height', height);

    // Add a group that will be transformed by zoom/pan
    const g = svg.append('g');

    // Enable zoom and pan on the SVG
    const zoom = d3.zoom()
        .scaleExtent([0.1, 4])
        .on('zoom', (event) => {
            g.attr('transform', event.transform);
        });
    svg.call(zoom);

    // Scale forces based on node count for better layout
    const nodeCount = data.nodes.length;
    const chargeStrength = Math.max(-200, -800 / Math.sqrt(nodeCount || 1));
    const linkDist = Math.max(60, Math.min(150, 2000 / Math.sqrt(nodeCount || 1)));

    // Create simulation
    const simulation = d3.forceSimulation(data.nodes)
        .force('link', d3.forceLink(data.links).id(d => d.id).distance(linkDist))
        .force('charge', d3.forceManyBody().strength(chargeStrength))
        .force('center', d3.forceCenter(width / 2, height / 2))
        .force('collision', d3.forceCollide().radius(d => (d.size || 5) + 20));

    // Draw links
    const link = g.append('g')
        .selectAll('line')
        .data(data.links)
        .enter()
        .append('line')
        .attr('stroke', '#999')
        .attr('stroke-opacity', 0.6)
        .attr('stroke-width', 1.5);

    // Draw nodes
    const node = g.append('g')
        .selectAll('g')
        .data(data.nodes)
        .enter()
        .append('g')
        .attr('class', 'node')
        .call(d3.drag()
            .on('start', dragstarted)
            .on('drag', dragged)
            .on('end', dragended));

    // Node circles
    node.append('circle')
        .attr('r', d => d.size || 5)
        .attr('fill', d => getNodeColor(d.node_type));

    // Node labels
    node.append('text')
        .text(d => d.label)
        .attr('x', 12)
        .attr('y', 4)
        .style('font-size', '12px')
        .style('fill', 'var(--text-primary)');

    // Node click handler
    node.on('click', (event, d) => {
        if (d.url) {
            window.location.href = d.url;
        }
    });

    // Update positions
    simulation.on('tick', () => {
        link
            .attr('x1', d => d.source.x)
            .attr('y1', d => d.source.y)
            .attr('x2', d => d.target.x)
            .attr('y2', d => d.target.y);

        node.attr('transform', d => `translate(${d.x},${d.y})`);
    });

    // After simulation settles, fit the graph to view
    simulation.on('end', () => {
        fitToView(svg, g, width, height, zoom);
    });

    function dragstarted(event, d) {
        if (!event.active) simulation.alphaTarget(0.3).restart();
        d.fx = d.x;
        d.fy = d.y;
    }

    function dragged(event, d) {
        d.fx = event.x;
        d.fy = event.y;
    }

    function dragended(event, d) {
        if (!event.active) simulation.alphaTarget(0);
        d.fx = null;
        d.fy = null;
    }
}

// Fit the graph content within the SVG viewport with padding.
function fitToView(svg, g, width, height, zoom) {
    const bounds = g.node().getBBox();
    if (bounds.width === 0 || bounds.height === 0) return;

    const padding = 40;
    const scale = Math.min(
        (width - padding * 2) / bounds.width,
        (height - padding * 2) / bounds.height,
        1.5
    );
    const tx = (width - bounds.width * scale) / 2 - bounds.x * scale;
    const ty = (height - bounds.height * scale) / 2 - bounds.y * scale;

    svg.transition().duration(750).call(
        zoom.transform,
        d3.zoomIdentity.translate(tx, ty).scale(scale)
    );
}

function getNodeColor(type) {
    const colors = {
        'entity': '#3b82f6',
        'concept': '#10b981',
        'source': '#f59e0b',
        'analysis': '#8b5cf6',
        'default': '#6b7280'
    };
    return colors[type] || colors['default'];
}

// Confirm before leaving unsaved editor
window.addEventListener('beforeunload', function(e) {
    const textarea = document.querySelector('.editor-textarea');
    if (textarea && textarea.dataset.dirty === 'true') {
        e.preventDefault();
        e.returnValue = '';
    }
});

// Mark editor as dirty on change
document.addEventListener('input', function(e) {
    if (e.target.classList.contains('editor-textarea')) {
        e.target.dataset.dirty = 'true';
    }
});
"#;

/// Render the base HTML page template.
pub fn render_base(title: &str, content: &str, wiki: Option<&Aiwiki>) -> String {
    let wiki_name = wiki.map(|w| w.config.name.as_str()).unwrap_or("AIWiki");

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{} - {}</title>
    <link rel="stylesheet" href="/aiwiki/static/css/aiwiki.css">
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github.min.css">
    <script src="https://d3js.org/d3.v7.min.js"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js"></script>
</head>
<body>
    <nav class="navbar">
        <a href="/aiwiki" class="navbar-brand">{} - AIWiki</a>
        <ul class="navbar-nav">
            <li><a href="/aiwiki">Home</a></li>
            <li><a href="/aiwiki/search">Search</a></li>
            <li><a href="/aiwiki/graph">Graph</a></li>
            <li><a href="/aiwiki/status">Status</a></li>
        </ul>
    </nav>
    <div class="main-container">
        {}
    </div>
    <script src="/aiwiki/static/js/aiwiki.js"></script>
</body>
</html>
"#,
        title, wiki_name, wiki_name, content
    )
}

/// Render markdown content to HTML.
pub fn render_markdown_to_html(content: &str, page_path: &str) -> String {
    // Resolve the directory prefix for relative links.
    // e.g. page_path = "concepts/terminal-user-interface.md" → dir = "concepts"
    let page_dir = Path::new(page_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");

    let mut html = String::new();

    // Escape HTML
    let content = content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");

    // Convert wiki links [[Page Name]] to markdown links
    let content = content.replace("[[", "[").replace("]]", "]");

    let mut in_list = false;
    let mut in_code = false;
    let mut skip_frontmatter = false;
    let mut frontmatter_seen = 0u8;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip YAML frontmatter
        if trimmed == "---" {
            frontmatter_seen += 1;
            if frontmatter_seen == 1 {
                skip_frontmatter = true;
                continue;
            } else if frontmatter_seen == 2 {
                skip_frontmatter = false;
                continue;
            }
        }
        if skip_frontmatter {
            continue;
        }

        // Code blocks
        if trimmed.starts_with("```") {
            if in_code {
                html.push_str("</code></pre>\n");
                in_code = false;
            } else {
                html.push_str("<pre><code>");
                in_code = true;
            }
            continue;
        }
        if in_code {
            html.push_str(trimmed);
            html.push('\n');
            continue;
        }

        // Close open list when hitting a non-list line
        if in_list && !trimmed.starts_with("- ") && !trimmed.starts_with("* ") {
            html.push_str("</ul>\n");
            in_list = false;
        }

        if trimmed.is_empty() {
            continue;
        }

        // Headers
        if let Some(rest) = trimmed.strip_prefix("### ") {
            html.push_str(&format!(
                "<h3>{}</h3>\n",
                process_links(rest, page_dir)
            ));
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            html.push_str(&format!(
                "<h2>{}</h2>\n",
                process_links(rest, page_dir)
            ));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            html.push_str(&format!(
                "<h1>{}</h1>\n",
                process_links(rest, page_dir)
            ));
        }
        // List items
        else if let Some(rest) = trimmed.strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            if !in_list {
                html.push_str("<ul>\n");
                in_list = true;
            }
            html.push_str(&format!(
                "<li>{}</li>\n",
                process_links(rest, page_dir)
            ));
        }
        // Bold text
        else if trimmed.starts_with("**") {
            html.push_str(&format!(
                "<p>{}</p>\n",
                process_links(trimmed, page_dir)
            ));
        }
        // Regular paragraphs
        else {
            html.push_str(&format!(
                "<p>{}</p>\n",
                process_links(trimmed, page_dir)
            ));
        }
    }

    if in_list {
        html.push_str("</ul>\n");
    }
    if in_code {
        html.push_str("</code></pre>\n");
    }

    html
}

/// Process markdown links in text, resolving relative paths to absolute wiki URLs.
fn process_links(text: &str, page_dir: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(start) = remaining.find('[') {
        // Push text before the link
        result.push_str(&remaining[..start]);

        let after_bracket = &remaining[start + 1..];
        // Find closing ] and ensure (url) follows
        if let Some(close) = after_bracket.find("](") {
            let link_text = &after_bracket[..close];
            let after_paren = &after_bracket[close + 2..];
            if let Some(end_paren) = after_paren.find(')') {
                let href = &after_paren[..end_paren];
                let resolved = resolve_wiki_link(href, page_dir);
                result.push_str(&format!(
                    "<a href=\"{}\" class=\"wiki-link\">{}</a>",
                    resolved, link_text
                ));
                remaining = &after_paren[end_paren + 1..];
                continue;
            }
        }
        // Not a valid link, push the [ and continue
        result.push('[');
        remaining = after_bracket;
    }
    result.push_str(remaining);

    // Process bold markers: **text** → <strong>text</strong>
    let result = result.replace("**", "<strong>");
    // Pair up strong tags (odd → open, even → close)
    let mut final_out = String::with_capacity(result.len());
    let mut open = true;
    for part in result.split("<strong>") {
        if !open {
            if open { // will never trigger — but toggle below handles it
            }
            final_out.push_str(if open { "<strong>" } else { "</strong>" });
        }
        final_out.push_str(part);
        open = !open;
    }

    final_out
}

/// Resolve a relative markdown link to an absolute `/aiwiki/page/...` path.
fn resolve_wiki_link(href: &str, page_dir: &str) -> String {
    // Already absolute
    if href.starts_with('/') || href.starts_with("http") {
        return href.to_string();
    }

    // Resolve ../  segments
    let mut parts: Vec<&str> = if page_dir.is_empty() {
        Vec::new()
    } else {
        page_dir.split('/').collect()
    };

    for segment in href.split('/') {
        if segment == ".." {
            parts.pop();
        } else if segment != "." && !segment.is_empty() {
            parts.push(segment);
        }
    }

    format!("/aiwiki/page/{}", parts.join("/"))
}

/// Render a wiki page.
pub fn render_markdown_page(path: &str, content: &str, wiki: &Aiwiki) -> String {
    let title = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled");

    let breadcrumbs = render_breadcrumbs(path);
    let html_content = render_markdown_to_html(content, path);

    let body = format!(
        r#"
        <div class="page-header">
            {}
            <h1 class="page-title">{}</h1>
            <div class="page-meta">
                <span>Path: {}</span>
                <span style="margin-left: 1rem;"></span>
                <a href="/aiwiki/edit/{}" class="edit-button">Edit</a>
            </div>
        </div>
        <div class="content">
            {}
        </div>
    "#,
        breadcrumbs,
        title,
        path,
        path,
        html_content
    );

    render_base(title, &body, Some(wiki))
}

/// Render breadcrumbs for a page.
fn render_breadcrumbs(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let mut html = String::from(r#"<div class="breadcrumbs"><a href="/aiwiki">Home</a>"#);

    let mut current_path = String::new();
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        html.push_str(r#"<span class="separator">/</span>"#);

        if i == parts.len() - 1 {
            html.push_str(&format!("{}", part));
        } else {
            current_path.push_str(part);
            current_path.push('/');
            html.push_str(&format!(
                "<a href=\"/aiwiki/page/{}/index.md\">{}</a>",
                current_path, part
            ));
        }
    }

    html.push_str("</div>");
    html
}

/// Render page edit form.
pub fn render_edit_page(path: &str, content: &str, wiki: &Aiwiki) -> String {
    let title = format!("Editing: {}", path);

    let escaped_content = html_escape(content);
    let cancel_path = path;

    let body = format!(
        r#"
        <div class="page-header">
            <h1 class="page-title">{}</h1>
        </div>
        <form action="/aiwiki/edit/{}" method="POST" class="editor-container">
            <textarea name="content" class="editor-textarea" placeholder="Write markdown here...">{}
            </textarea>
            <div class="editor-actions">
                <button type="submit" class="btn btn-primary">Save Changes</button>
                <a href="/aiwiki/page/{}" class="btn btn-secondary">Cancel</a>
            </div>
        </form>
    "#,
        title,
        path,
        escaped_content,
        cancel_path
    );

    render_base(&title, &body, Some(wiki))
}

/// Escape HTML special characters.
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render search results page.
pub fn render_search_page(query: &str, results: &[SearchResult], wiki: &Aiwiki) -> String {
    let title = if query.is_empty() {
        "Search".to_string()
    } else {
        format!("Search: {}", query)
    };

    let results_html: String = results
        .iter()
        .map(|r| {
            format!(
                r#"
                <li class="search-result">
                    <div class="search-result-title">
                        <a href="/aiwiki/page/{}">{}</a>
                    </div>
                    <div class="search-result-excerpt">{}</div>
                    <div class="search-result-meta">{} • {} words</div>
                </li>
            "#,
                r.path, r.title, r.excerpt, r.page_type, r.word_count
            )
        })
        .collect();

    let body = format!(
        r#"
        <div class="page-header">
            <h1 class="page-title">Search</h1>
        </div>
        <form action="/aiwiki/search" method="GET" class="search-box" id="search-form">
            <input 
                type="text" 
                name="q" 
                id="search-input"
                value="{}" 
                placeholder="Search wiki pages..."
                autocomplete="off"
            >
            <button type="submit">Search</button>
        </form>
        
        {}
        
        <ul class="search-results">
            {}
        </ul>
    "#,
        html_escape(query),
        if results.is_empty() && !query.is_empty() {
            "<p>No results found.</p>".to_string()
        } else {
            format!("<p>Found {} results.</p>", results.len())
        },
        results_html
    );

    render_base(&title, &body, Some(wiki))
}

/// Render graph visualization page.
pub fn render_graph_page(_filter: Option<&str>) -> String {
    let title = "Page Graph";

    let body = format!(
        r#"
        <div class="page-header">
            <h1 class="page-title">Page Graph</h1>
            <p class="page-meta">
                Visualize relationships between wiki pages. 
                Click nodes to navigate. Drag to rearrange. Scroll to zoom.
            </p>
        </div>
        <div class="graph-container" id="graph-viz">
            <div style="text-align: center; padding-top: 2rem; color: var(--text-muted);">
                Loading graph visualization...
            </div>
        </div>
    "#
    );

    render_base(title, &body, None)
}

/// Render status dashboard.
pub fn render_status_page(wiki: &Aiwiki) -> String {
    let stats = wiki.state.stats();
    let title = "Wiki Status";

    let body = format!(
        r#"
        <div class="page-header">
            <h1 class="page-title">{}</h1>
            <div class="page-meta">
                Version {} • Sync Mode: {:?}
            </div>
        </div>
        
        <div class="status-grid">
            <div class="status-card">
                <div class="status-card-title">Source Files</div>
                <div class="status-card-value">{}</div>
                <div class="status-card-subtitle">Files in raw/</div>
            </div>
            
            <div class="status-card">
                <div class="status-card-title">Wiki Pages</div>
                <div class="status-card-value">{}</div>
                <div class="status-card-subtitle">Generated pages</div>
            </div>
            
            <div class="status-card">
                <div class="status-card-title">Last Sync</div>
                <div class="status-card-value">{}</div>
                <div class="status-card-subtitle">{}</div>
            </div>
            
            <div class="status-card">
                <div class="status-card-title">Status</div>
                <div class="status-card-value">{}</div>
                <div class="status-card-subtitle">AIWiki is {}</div>
            </div>
        </div>
        
        <h2>Actions</h2>
        <p>
            <a href="/aiwiki/sync" class="btn btn-primary">Sync Now</a>
            <span style="margin: 0 0.5rem;"></span>
            <a href="/aiwiki" class="btn btn-secondary">View Wiki</a>
        </p>
    "#,
        wiki.config.name,
        wiki.config.version,
        wiki.config.sync_mode,
        stats.total_sources,
        stats.total_pages,
        stats.last_sync.map(|t| t.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "Never".to_string()),
        stats.last_sync.map(|t| t.format("%H:%M").to_string())
            .unwrap_or_default(),
        if wiki.config.enabled { "🟢" } else { "🔴" },
        if wiki.config.enabled { "active" } else { "disabled" }
    );

    render_base(title, &body, Some(wiki))
}

/// Render index page (directory listing).
pub fn render_index_page(wiki: &Aiwiki) -> String {
    let title = "Wiki Index";

    let body = format!(
        r#"
        <div class="page-header">
            <h1 class="page-title">{}</h1>
            <p class="page-meta">Browse wiki pages by category</p>
        </div>
        
        <div class="content">
            <h2><a href="/aiwiki/page/entities">Entities</a></h2>
            <p>People, places, organizations extracted from sources.</p>
            
            <h2><a href="/aiwiki/page/concepts">Concepts</a></h2>
            <p>Ideas, topics, theories, and abstract concepts.</p>
            
            <h2><a href="/aiwiki/page/sources">Sources</a></h2>
            <p>Summaries of ingested source documents.</p>
            
            <h2><a href="/aiwiki/page/analyses">Analyses</a></h2>
            <p>Derived content: comparisons, Q&A, and synthesized knowledge.</p>
        </div>
    "#,
        wiki.config.name
    );

    render_base(title, &body, Some(wiki))
}

/// Render a directory listing page.
pub fn render_directory_page(
    dir_path: &str,
    entries: &[super::DirectoryEntry],
    wiki: &Aiwiki,
) -> String {
    use std::fmt::Write;

    let dir_name = dir_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(dir_path);
    let title = format!("{} — {}", capitalise(dir_name), wiki.config.name);
    let breadcrumbs = render_breadcrumbs(dir_path);

    let mut items = String::new();
    for entry in entries {
        if entry.is_dir {
            let _ = write!(
                items,
                r#"<li class="dir-entry"><a href="/aiwiki/page/{}">📁 {}</a></li>"#,
                html_escape(&entry.link),
                html_escape(&entry.title),
            );
        } else {
            let _ = write!(
                items,
                r#"<li class="page-entry"><a href="/aiwiki/page/{}">📄 {}</a></li>"#,
                html_escape(&entry.link),
                html_escape(&entry.title),
            );
        }
    }

    let count_label = if entries.len() == 1 { "item" } else { "items" };

    let body = format!(
        r#"
        {breadcrumbs}
        <div class="page-header">
            <h1 class="page-title">{dir_name}</h1>
            <p class="page-meta">{count} {count_label}</p>
        </div>
        <div class="content">
            <ul class="directory-listing">
                {items}
            </ul>
        </div>
        "#,
        breadcrumbs = breadcrumbs,
        dir_name = html_escape(capitalise(dir_name).as_str()),
        count = entries.len(),
        count_label = count_label,
        items = items,
    );

    render_base(&title, &body, Some(wiki))
}

/// Capitalise the first letter of a string.
fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Render setup page (when wiki not initialized).
pub fn render_setup_page() -> String {
    let title = "Setup AIWiki";

    let body = r#"
        <div class="setup-container">
            <h1 class="setup-title">AIWiki Not Initialized</h1>
            <p class="setup-description">
                This project doesn't have an AIWiki yet. Initialize it to start building your knowledge base.
            </p>
            
            <div class="setup-steps">
                <ol>
                    <li>Run <code>/aiwiki init</code> in the TUI to initialize the wiki structure.</li>
                    <li>Add documents to <code>aiwiki/raw/</code> directory.</li>
                    <li>Run <code>/aiwiki sync</code> to process and generate wiki pages.</li>
                    <li>Browse your wiki here or use <code>/aiwiki</code> to open in browser.</li>
                </ol>
            </div>
            
            <p>
                <a href="/" class="btn btn-secondary">Back to Home</a>
            </p>
        </div>
    "#;

    render_base(title, body, None)
}

/// Render error page.
pub fn render_error_page(error_title: &str, message: &str) -> String {
    let body = format!(
        r#"
        <div class="error-container">
            <h1 class="error-title">{}</h1>
            <p class="error-message">{}</p>
            <p>
                <a href="/aiwiki" class="btn btn-secondary">Back to Wiki</a>
            </p>
        </div>
    "#,
        error_title, message
    );

    render_base("Error", &body, None)
}

/// Render 404 not found page.
pub fn render_not_found_page(path: &str, wiki: &Aiwiki) -> String {
    let body = format!(
        r#"
        <div class="error-container">
            <h1 class="error-title">Page Not Found</h1>
            <p class="error-message">
                The page "<code>{}</code>" doesn't exist yet.
            </p>
            <p>
                <a href="/aiwiki/edit/{}" class="btn btn-primary">Create This Page</a>
                <span style="margin: 0 0.5rem;"></span>
                <a href="/aiwiki" class="btn btn-secondary">Back to Wiki</a>
            </p>
        </div>
    "#,
        path, path
    );

    render_base("Not Found", &body, Some(wiki))
}

/// Create a template for a new page.
pub fn create_page_template(path: &str) -> String {
    let title = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("New Page");

    // Convert filename to title (replace - and _ with spaces, capitalize)
    let title = title
        .replace('-', " ")
        .replace('_', " ");

    let slug = title.to_lowercase().replace(' ', "-");

    format!(
        r#"---
title: {}
slug: {}
date: {}
type: page
---

# {}

Write your content here...
"#,
        title,
        slug,
        chrono::Local::now().format("%Y-%m-%d"),
        title
    )
}

/// Search result item.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    /// Path to the page.
    pub path: String,
    /// Page title.
    pub title: String,
    /// Search excerpt.
    pub excerpt: String,
    /// Page type (entity, concept, source, analysis).
    pub page_type: String,
    /// Word count.
    pub word_count: usize,
    /// Relevance score.
    pub score: f32,
}
