//! HTML templates and CSS/JS for AIWiki web interface.

use crate::Aiwiki;
use crate::web::handlers::{EntitySidebarData, EntityTypeInfo, PageInfo};
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

/* Mermaid diagram styling */
.mermaid {
    border-radius: 8px;
    padding: 1rem;
    margin: 1rem 0;
    text-align: center;
}

.mermaid svg {
    max-width: 100%;
    height: auto;
}

/* Entity page sidebar layout */
.entity-page-layout {
    display: flex;
    gap: 2rem;
    align-items: flex-start;
}

.entity-page-layout > .content {
    flex: 1;
    min-width: 0;
}

.entity-sidebar {
    width: 240px;
    flex-shrink: 0;
    position: sticky;
    top: 1rem;
    max-height: calc(100vh - 6rem);
    overflow-y: auto;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
    font-size: 0.85rem;
}

.sidebar-section {
    margin-bottom: 1.25rem;
}

.sidebar-section:last-child {
    margin-bottom: 0;
}

.sidebar-heading {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    margin-bottom: 0.5rem;
    padding-bottom: 0.25rem;
    border-bottom: 1px solid var(--border-color);
}

.sidebar-types {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
}

.sidebar-type-link {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    text-decoration: none;
    color: var(--text-secondary);
    transition: background 0.15s ease;
}

.sidebar-type-link:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
}

.sidebar-all-link {
    font-weight: 600;
    color: var(--accent-primary);
    margin-bottom: 0.15rem;
}

.sidebar-type-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.sidebar-type-count {
    font-size: 0.75rem;
    color: var(--text-muted);
    background: var(--bg-tertiary);
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
    min-width: 1.4rem;
    text-align: center;
}

.sidebar-alpha {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
}

.sidebar-letter {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.75rem;
    height: 1.75rem;
    border-radius: 4px;
    text-decoration: none;
    font-weight: 600;
    font-size: 0.8rem;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    transition: all 0.15s ease;
}

.sidebar-letter:hover {
    background: var(--accent-primary);
    color: white;
}

.sidebar-entities-section {
    max-height: none;
}

.sidebar-entities {
    overflow-y: visible;
}

.sidebar-letter-group {
    margin-top: 0.5rem;
}

.sidebar-letter-group:first-child {
    margin-top: 0;
}

.sidebar-letter-heading {
    font-weight: 700;
    font-size: 0.9rem;
    color: var(--text-primary);
    display: block;
    padding: 0.15rem 0;
}

.sidebar-entity-list {
    list-style: none;
    padding: 0;
    margin: 0 0 0.25rem 0;
}

.sidebar-entity-link {
    display: block;
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    text-decoration: none;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: background 0.15s ease;
}

.sidebar-entity-link:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
}

.sidebar-entity-link.active {
    background: var(--accent-primary);
    color: white;
    font-weight: 600;
}

@media (max-width: 768px) {
    .entity-page-layout {
        flex-direction: column;
    }
    .entity-sidebar {
        width: 100%;
        position: static;
        max-height: none;
    }
    .sidebar-entities-section {
        display: none;
    }
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
    height: 840px;
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
          <script src="https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.min.js"></script>
          <script>
              mermaid.initialize({{
                  startOnLoad: false,
                  theme: window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'default',
                  securityLevel: 'loose'
              }});
              document.addEventListener('DOMContentLoaded', function() {{
                  var els = document.querySelectorAll('.mermaid');
                  els.forEach(function(el, i) {{
                      var text = el.textContent;
                      el.textContent = '';
                      mermaid.render('mermaid-graph-' + i, text).then(function(result) {{
                          el.innerHTML = result.svg;
                      }}).catch(function(err) {{
                          var safe = text.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
                          el.innerHTML = '<details style="text-align:left;margin:1rem 0;">'
                              + '<summary style="color:#f88;cursor:pointer;font-weight:600;">⚠ Diagram error: '
                              + err.message.split('\\n')[0] + '</summary>'
                              + '<pre style="background:var(--bg-secondary);padding:0.75rem;border-radius:6px;'
                              + 'margin-top:0.5rem;font-size:0.8rem;overflow-x:auto;white-space:pre-wrap;">'
                              + safe + '</pre></details>';
                      }});
                  }});
              }});
          </script>
      </head><body>
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

    // Convert wiki links [[Page Name]] to markdown links
    let content = content.replace("[[", "[").replace("]]", "]");

    let mut in_list = false;
    let mut in_code = false;
    let mut in_mermaid = false;
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
                if in_mermaid {
                    html.push_str("</div>\n");
                } else {
                    html.push_str("</code></pre>\n");
                }
                in_code = false;
                in_mermaid = false;
            } else {
                // Check if this is a mermaid diagram
                let lang = trimmed.trim_start_matches('`').trim();
                if lang == "mermaid" {
                    in_mermaid = true;
                    html.push_str("<div class=\"mermaid\">\n");
                } else {
                    html.push_str("<pre><code>");
                }
                in_code = true;
            }
            continue;
        }
        if in_code {
            if in_mermaid {
                // HTML-escape mermaid content so angle brackets in labels
                // (e.g. Arc<AppState>) aren't parsed as HTML tags.
                // Mermaid v10 reads textContent which auto-decodes entities.
                html.push_str(&html_escape(line));
                html.push('\n');
            } else {
                // Regular code blocks need HTML escaping
                html.push_str(&html_escape(trimmed));
                html.push('\n');
            }
            continue;
        }

        // HTML-escape the line for all non-code content
        let trimmed = &html_escape(trimmed);

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
            html.push_str(&format!("<h3>{}</h3>\n", process_links(rest, page_dir)));
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            html.push_str(&format!("<h2>{}</h2>\n", process_links(rest, page_dir)));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            html.push_str(&format!("<h1>{}</h1>\n", process_links(rest, page_dir)));
        }
        // List items
        else if let Some(rest) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            if !in_list {
                html.push_str("<ul>\n");
                in_list = true;
            }
            html.push_str(&format!("<li>{}</li>\n", process_links(rest, page_dir)));
        }
        // Bold text
        else if trimmed.starts_with("**") {
            html.push_str(&format!("<p>{}</p>\n", process_links(trimmed, page_dir)));
        }
        // Regular paragraphs
        else {
            html.push_str(&format!("<p>{}</p>\n", process_links(trimmed, page_dir)));
        }
    }

    if in_list {
        html.push_str("</ul>\n");
    }
    if in_mermaid {
        html.push_str("</div>\n");
    }
    if in_code && !in_mermaid {
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
///
/// When `sidebar` is `Some`, the page is rendered with entity navigation
/// containing type filters and alphabetical quick-jump links.
pub fn render_markdown_page(
    path: &str,
    content: &str,
    wiki: &Aiwiki,
    sidebar: Option<&EntitySidebarData>,
) -> String {
    let title = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled");

    let breadcrumbs = render_breadcrumbs(path);
    let html_content = render_markdown_to_html(content, path);

    // Find source attribution if this page was generated from a source file
    let source_attribution = find_page_source(path, wiki);
    let source_html = source_attribution
        .map(|src| {
            format!(
                r#" <span class="source-attribution">Source: {}</span>"#,
                html_escape(&src)
            )
        })
        .unwrap_or_default();

    let sidebar_html = sidebar
        .map(|sb| render_entity_sidebar(sb, path))
        .unwrap_or_default();

    let wrapper_class = if sidebar.is_some() {
        "entity-page-layout"
    } else {
        ""
    };

    let body = format!(
        r#"
        <div class="page-header">
            {}
            <h1 class="page-title">{}</h1>
            <div class="page-meta">
                <span>Path: {}</span>
                {}
                <span style="margin-left: 1rem;"></span>
                <a href="/aiwiki/edit/{}" class="edit-button">Edit</a>
            </div>
        </div>
        <div class="{}">
            {}
            <div class="content">
                {}
            </div>
        </div>
        <style>
        .source-attribution {{ 
            background: var(--bg-tertiary); 
            padding: 0.25rem 0.5rem; 
            border-radius: 4px; 
            font-size: 0.85rem; 
            color: var(--text-secondary);
            margin-left: 1rem;
        }}
        </style>
    "#,
        breadcrumbs, title, path, source_html, path, wrapper_class, sidebar_html, html_content
    );

    render_base(title, &body, Some(wiki))
}

/// Render the entity navigation sidebar.
///
/// Contains two sections: entity type filter links and alphabetical
/// quick-jump using single initial letters.
fn render_entity_sidebar(sidebar: &EntitySidebarData, current_path: &str) -> String {
    // Build type filter links
    let types_html: String = sidebar
        .types
        .iter()
        .map(|(name, count)| {
            format!(
                r#"<a href="/aiwiki/entities/type/{}" class="sidebar-type-link">
                    <span class="sidebar-type-name">{}</span>
                    <span class="sidebar-type-count">{}</span>
                </a>"#,
                html_escape(&name.to_lowercase().replace(' ', "-")),
                html_escape(name),
                count,
            )
        })
        .collect();

    // Build alphabetical nav
    let alpha_html: String = sidebar
        .letters
        .iter()
        .map(|(ch, count)| {
            format!(
                r##"<a href="#letter-{}" class="sidebar-letter" title="{} entities">{}</a>"##,
                ch.to_lowercase().next().unwrap_or(*ch),
                count,
                ch,
            )
        })
        .collect();

    // Build entity list grouped by initial letter
    let mut current_letter: Option<char> = None;
    let mut entity_list = String::new();
    for entry in &sidebar.entities {
        let initial = entry
            .title
            .chars()
            .next()
            .map(|c| c.to_uppercase().next().unwrap_or(c))
            .unwrap_or('?');

        if current_letter != Some(initial) {
            if current_letter.is_some() {
                entity_list.push_str("</ul>");
            }
            entity_list.push_str(&format!(
                r##"<div class="sidebar-letter-group" id="letter-{}">
                    <span class="sidebar-letter-heading">{}</span>
                </div>
                <ul class="sidebar-entity-list">"##,
                initial.to_lowercase().next().unwrap_or(initial),
                initial,
            ));
            current_letter = Some(initial);
        }

        let is_current = entry.path == current_path;
        let active_class = if is_current { " active" } else { "" };
        entity_list.push_str(&format!(
            r#"<li><a href="/aiwiki/page/{}" class="sidebar-entity-link{}">{}</a></li>"#,
            html_escape(&entry.path),
            active_class,
            html_escape(&entry.title),
        ));
    }
    if current_letter.is_some() {
        entity_list.push_str("</ul>");
    }

    format!(
        r#"<aside class="entity-sidebar">
            <div class="sidebar-section">
                <h3 class="sidebar-heading">Entity Types</h3>
                <div class="sidebar-types">
                    <a href="/aiwiki/entities/types" class="sidebar-type-link sidebar-all-link">
                        <span class="sidebar-type-name">All Types</span>
                    </a>
                    {}
                </div>
            </div>
            <div class="sidebar-section">
                <h3 class="sidebar-heading">A–Z</h3>
                <div class="sidebar-alpha">
                    {}
                </div>
            </div>
            <div class="sidebar-section sidebar-entities-section">
                <h3 class="sidebar-heading">Entities</h3>
                <div class="sidebar-entities">
                    {}
                </div>
            </div>
        </aside>"#,
        types_html, alpha_html, entity_list,
    )
}

/// Find the source file that generated this wiki page.
fn find_page_source(page_path: &str, wiki: &Aiwiki) -> Option<String> {
    // Check state to find which source file generated this page
    for (source_key, file_state) in &wiki.state.files {
        for generated_page in &file_state.generated_pages {
            if generated_page == page_path {
                // Determine if it's from raw/ or a referenced folder
                if source_key.starts_with("ref:") {
                    // Extract source folder and file from ref: key
                    if let Some(pos) = source_key.find('/') {
                        let source_path = &source_key[4..pos]; // After "ref:" and up to next /
                        let file_path = &source_key[pos + 1..];
                        return Some(format!("{}/{}", source_path, file_path));
                    }
                } else {
                    return Some(format!("raw/{}", source_key));
                }
            }
        }
    }
    None
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
        title, path, escaped_content, cancel_path
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

    // Count raw and referenced files separately
    let raw_count = wiki
        .state
        .files
        .keys()
        .filter(|k| !k.starts_with("ref:"))
        .count();
    let ref_count = wiki
        .state
        .files
        .keys()
        .filter(|k| k.starts_with("ref:"))
        .count();

    // Build Sources section
    let mut sources_section = String::new();
    if !wiki.config.sources.is_empty() {
        sources_section.push_str(
            r#"<div class="sources-section"><h2>Source Folders</h2><div class="sources-grid">"#,
        );
        for source in &wiki.config.sources {
            let tracked_count = wiki
                .state
                .files
                .keys()
                .filter(|k| k.starts_with(&format!("ref:{}/", source.path)))
                .count();
            let label = source.label.as_deref().unwrap_or("-");
            let patterns = source.patterns.join(", ");
            let status_class = if source.enabled {
                "enabled"
            } else {
                "disabled"
            };
            let status_text = if source.enabled {
                "✓ Enabled"
            } else {
                "✗ Disabled"
            };
            sources_section.push_str(&format!(
                r#"<div class="source-card {status_class}"><div class="source-header"><span class="source-path">{}</span><span class="source-status">{}</span></div><div class="source-label">{}</div><div class="source-patterns">Patterns: {}</div><div class="source-count">{} files tracked</div></div>"#,
                source.path, status_text, label, patterns, tracked_count
            ));
        }
        sources_section.push_str("</div></div>");
    }

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
                <div class="status-card-title">Raw Files</div>
                <div class="status-card-value">{}</div>
                <div class="status-card-subtitle">Files in raw/</div>
            </div>
            
            <div class="status-card">
                <div class="status-card-title">Ref Files</div>
                <div class="status-card-value">{}</div>
                <div class="status-card-subtitle">Files from source folders</div>
            </div>
            
            <div class="status-card">
                <div class="status-card-title">Wiki Pages</div>
                <div class="status-card-value">{}</div>
                <div class="status-card-subtitle">Generated pages</div>
            </div>
            
            <div class="status-card">
                <div class="status-card-title">Sources</div>
                <div class="status-card-value">{}</div>
                <div class="status-card-subtitle">{} folder(s)</div>
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
        
        {sources_section}
        
        <h2>Actions</h2>
        <p>
            <a href="/aiwiki/sync" class="btn btn-primary">Sync Now</a>
            <span style="margin: 0 0.5rem;"></span>
            <a href="/aiwiki" class="btn btn-secondary">View Wiki</a>
        </p>
        
        <style>
        .sources-section {{ margin: 2rem 0; }}
        .sources-grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 1rem; margin-top: 1rem; }}
        .source-card {{ background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1rem; }}
        .source-card.enabled {{ border-left: 4px solid var(--success); }}
        .source-card.disabled {{ border-left: 4px solid var(--text-muted); opacity: 0.7; }}
        .source-header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; }}
        .source-path {{ font-weight: 600; font-size: 1.1rem; }}
        .source-status {{ font-size: 0.85rem; color: var(--text-secondary); }}
        .source-label {{ color: var(--text-muted); font-size: 0.9rem; margin-bottom: 0.5rem; }}
        .source-patterns {{ font-family: var(--font-mono); font-size: 0.8rem; background: var(--bg-tertiary); padding: 0.25rem 0.5rem; border-radius: 4px; margin-bottom: 0.5rem; }}
        .source-count {{ font-size: 0.85rem; color: var(--text-secondary); }}
        </style>
    "#,
        wiki.config.name,
        wiki.config.version,
        wiki.config.sync_mode,
        raw_count,
        ref_count,
        stats.total_pages,
        wiki.config.sources.len(),
        wiki.config.enabled_sources().count(),
        stats
            .last_sync
            .map(|t| t.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "Never".to_string()),
        stats
            .last_sync
            .map(|t| t.format("%H:%M").to_string())
            .unwrap_or_default(),
        if wiki.config.enabled { "🟢" } else { "🔴" },
        if wiki.config.enabled {
            "active"
        } else {
            "disabled"
        },
        sources_section = sources_section
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

/// Render sources list page.
pub fn render_sources_page(wiki: &Aiwiki) -> String {
    let title = "Source Folders";

    let mut sources_html = String::new();

    if wiki.config.sources.is_empty() {
        sources_html.push_str(r#"<div class="empty-state"><p>No source folders registered.</p><p>Use <code>/aiwiki sources add &lt;path&gt;</code> to add a source folder.</p></div>"#);
    } else {
        sources_html.push_str(r#"<div class="sources-grid">"#);
        for source in &wiki.config.sources {
            let tracked_count = wiki
                .state
                .files
                .keys()
                .filter(|k| k.starts_with(&format!("ref:{}/", source.path)))
                .count();
            let label = source.label.as_deref().unwrap_or("-");
            let patterns = source.patterns.join(", ");
            let status_class = if source.enabled {
                "enabled"
            } else {
                "disabled"
            };
            let status_text = if source.enabled {
                "✓ Enabled"
            } else {
                "✗ Disabled"
            };

            sources_html.push_str(&format!(
                r#"<a href="/aiwiki/source/{}" class="source-card {status_class}">
                    <div class="source-header">
                        <span class="source-path">{}</span>
                        <span class="source-status">{}</span>
                    </div>
                    <div class="source-label">{}</div>
                    <div class="source-patterns">{}</div>
                    <div class="source-count">{} files</div>
                </a>
                "#,
                source.path, source.path, status_text, label, patterns, tracked_count
            ));
        }
        sources_html.push_str("</div>");
    }

    let body = format!(
        r#"
        <div class="page-header">
            <h1 class="page-title">Source Folders</h1>
            <div class="page-meta">{} registered sources</div>
        </div>
        
        <div class="content">
            {}
        </div>
        
        <style>
        .sources-grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 1rem; margin-top: 1rem; }}
        .source-card {{ background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1.5rem; text-decoration: none; color: inherit; display: block; transition: transform 0.1s, box-shadow 0.1s; }}
        .source-card:hover {{ transform: translateY(-2px); box-shadow: 0 4px 12px rgba(0,0,0,0.1); }}
        .source-card.enabled {{ border-left: 4px solid var(--success); }}
        .source-card.disabled {{ border-left: 4px solid var(--text-muted); opacity: 0.7; }}
        .source-header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; }}
        .source-path {{ font-weight: 600; font-size: 1.2rem; color: var(--text-primary); }}
        .source-status {{ font-size: 0.85rem; color: var(--text-secondary); }}
        .source-label {{ color: var(--text-muted); font-size: 0.95rem; margin-bottom: 0.75rem; }}
        .source-patterns {{ font-family: var(--font-mono); font-size: 0.85rem; background: var(--bg-tertiary); padding: 0.35rem 0.6rem; border-radius: 4px; margin-bottom: 0.75rem; color: var(--text-secondary); }}
        .source-count {{ font-size: 0.9rem; color: var(--accent-primary); font-weight: 500; }}
                  .empty-state {{ text-align: center; padding: 3rem; color: var(--text-muted); }}
                  .empty-state p {{ margin-bottom: 1rem; }}
                  .empty-state code {{ background: var(--bg-tertiary); padding: 0.2rem 0.4rem; border-radius: 4px; }}
                  </style>
              "#,
        wiki.config.sources.len(),
        sources_html
    );

    render_base(title, &body, Some(wiki))
}

/// Render entity types list page.
pub fn render_entity_types_page(types: &[(EntityTypeInfo, usize)], wiki: &Aiwiki) -> String {
    let title = "Entity Types";

    let types_html: String = types
        .iter()
        .map(|(type_info, count)| {
            format!(
                r#"
                      <a href="/aiwiki/entities/type/{}" class="entity-type-card">
                          <div class="entity-type-header">
                              <span class="entity-type-name">{}</span>
                              <span class="entity-type-count">{} entities</span>
                          </div>
                      </a>
                      "#,
                html_escape(&type_info.slug),
                html_escape(&type_info.name),
                count
            )
        })
        .collect();

    let body = format!(
        r#"
              <div class="page-header">
                  <h1 class="page-title">Entity Types</h1>
                  <div class="page-meta">{} unique types found</div>
              </div>
              
              <div class="content">
                  {}
              </div>
              
              <style>
              .entity-types-grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 1rem; margin-top: 1rem; }}
              .entity-type-card {{ background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1.25rem; text-decoration: none; color: inherit; display: block; transition: all 0.2s ease; }}
              .entity-type-card:hover {{ transform: translateY(-2px); box-shadow: 0 4px 12px rgba(0,0,0,0.1); border-color: var(--accent-primary); }}
              .entity-type-header {{ display: flex; justify-content: space-between; align-items: center; }}
              .entity-type-name {{ font-weight: 600; font-size: 1.1rem; color: var(--text-primary); }}
              .entity-type-count {{ font-size: 0.875rem; color: var(--accent-primary); background: var(--bg-tertiary); padding: 0.25rem 0.5rem; border-radius: 4px; }}
              .empty-state {{ text-align: center; padding: 3rem; color: var(--text-muted); }}
              </style>
              "#,
        types.len(),
        if types.is_empty() {
            r#"
                  <div class="empty-state">
                      <p>No entity types found.</p>
                      <p>Entities with <code>entity_type</code> frontmatter will appear here.</p>
                  </div>
                  "#
            .to_string()
        } else {
            format!(r#"<div class="entity-types-grid">{}</div>"#, types_html)
        }
    );

    render_base(title, &body, Some(wiki))
}

/// Render entities by type page.
pub fn render_entities_by_type_page(
    entity_type: &str,
    entities: &[PageInfo],
    wiki: &Aiwiki,
) -> String {
    let title = format!("Entities: {}", entity_type);

    let entities_html: String = entities
              .iter()
              .map(|e| {
                  format!(
                      r#"
                      <li class="entity-list-item">
                          <div class="entity-title">
                              <a href="/aiwiki/page/{}" style="text-decoration:none;color:inherit;">{}</a>
                          </div>
                          <div class="entity-meta">{} words{}</div>
                      </li>
                      "#,
                      html_escape(&e.path),
                      html_escape(&e.title),
                      e.word_count,
                      e.modified
                          .as_ref()
                          .map(|m| format!(" • {}", m))
                          .unwrap_or_default()
                  )
              })
              .collect();

    let body = format!(
        r#"
              <div class="page-header">
                  <h1 class="page-title">Entities: {}</h1>
                  <div class="page-meta">
                      <a href="/aiwiki/entities/types">← Back to Entity Types</a>
                      <span style="margin: 0 1rem;">|</span>
                      <span>{} entities</span>
                  </div>
              </div>
              
              <div class="content">
                  {}
              </div>
              
              <style>
              .entity-list {{ list-style: none; padding: 0; margin: 1rem 0; }}
              .entity-list-item {{ background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1rem; margin-bottom: 0.75rem; transition: all 0.2s ease; }}
              .entity-list-item:hover {{ border-color: var(--accent-primary); transform: translateX(4px); }}
              .entity-title {{ font-weight: 600; font-size: 1.1rem; margin-bottom: 0.25rem; }}
              .entity-title a {{ color: var(--text-primary); text-decoration: none; }}
              .entity-title a:hover {{ color: var(--accent-primary); }}
              .entity-meta {{ font-size: 0.875rem; color: var(--text-muted); }}
              .empty-state {{ text-align: center; padding: 3rem; color: var(--text-muted); }}
              </style>
              "#,
        html_escape(entity_type),
        entities.len(),
        if entities.is_empty() {
            r#"
                  <div class="empty-state">
                      <p>No entities found of this type.</p>
                  </div>
                  "#
            .to_string()
        } else {
            format!(r#"<ul class="entity-list">{}</ul>"#, entities_html)
        }
    );

    render_base(&title, &body, Some(wiki))
}
/// Render source detail page - shows pages generated from this source.
pub fn render_source_detail_page(source_path: &str, wiki: &Aiwiki) -> String {
    // Try to find the source in config
    let source = wiki.config.sources.iter().find(|s| s.path == source_path);

    let (title, content) = match source {
        Some(src) => {
            let label = src.label.as_deref().unwrap_or("Source");
            let tracked_count = wiki
                .state
                .files
                .keys()
                .filter(|k| k.starts_with(&format!("ref:{}/", source_path)))
                .count();

            // Find generated pages from this source
            let mut generated_pages: Vec<(String, String)> = Vec::new();
            for (key, file_state) in &wiki.state.files {
                if key.starts_with(&format!("ref:{}/", source_path)) {
                    for page in &file_state.generated_pages {
                        let page_name = Path::new(page)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(page)
                            .to_string();
                        generated_pages.push((page.clone(), page_name));
                    }
                }
            }

            let mut pages_html = String::new();
            if generated_pages.is_empty() {
                pages_html.push_str(r#"<p class="no-pages">No pages generated from this source yet. Run <code>/aiwiki sync</code> to process the source.</p>"#);
            } else {
                pages_html.push_str("<ul class='page-list'>");
                for (path, name) in generated_pages {
                    pages_html.push_str(&format!(
                        r#"<li><a href="/aiwiki/page/{}">{}</a></li>"#,
                        path, name
                    ));
                }
                pages_html.push_str("</ul>");
            }

            let patterns = src.patterns.join(", ");
            let status = if src.enabled { "Enabled" } else { "Disabled" };

            let body = format!(
                r#"
                <div class="source-info">
                    <div class="info-row"><span>Label:</span><span>{}</span></div>
                    <div class="info-row"><span>Path:</span><span>{}</span></div>
                    <div class="info-row"><span>Patterns:</span><span>{}</span></div>
                    <div class="info-row"><span>Status:</span><span>{}</span></div>
                    <div class="info-row"><span>Tracked Files:</span><span>{}</span></div>
                </div>
                <h2>Generated Pages</h2>
                {}
                <style>
                .source-info {{ background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1.5rem; margin-bottom: 2rem; }}
                .info-row {{ display: flex; justify-content: space-between; padding: 0.75rem 0; border-bottom: 1px solid var(--border-color); }}
                .info-row:last-child {{ border-bottom: none; }}
                .info-row span:first-child {{ color: var(--text-muted); }}
                .page-list {{ list-style: none; padding: 0; }}
                .page-list li {{ padding: 0.75rem; background: var(--bg-secondary); border-radius: 6px; margin-bottom: 0.5rem; }}
                .page-list li:hover {{ background: var(--bg-tertiary); }}
                .page-list a {{ color: var(--text-primary); text-decoration: none; }}
                .no-pages {{ color: var(--text-muted); text-align: center; padding: 2rem; }}
                .no-pages code {{ background: var(--bg-tertiary); padding: 0.2rem 0.4rem; border-radius: 4px; }}
                </style>
                "#,
                label, source_path, patterns, status, tracked_count, pages_html
            );
            (format!("{}", source_path), body)
        }
        None => {
            let body = format!(
                r#"<p class="error">Source folder "{}" not found.</p>
                <p><a href="/aiwiki/sources">← Back to sources list</a></p>
                <style>.error {{ color: var(--error); padding: 2rem; }}</style>"#,
                source_path
            );
            ("Source Not Found".to_string(), body)
        }
    };

    let full_body = format!(
        r#"
        <div class="page-header">
            <h1 class="page-title">{}</h1>
            <div class="page-meta"><a href="/aiwiki/sources">← Back to sources</a></div>
        </div>
        <div class="content">{}</div>
        "#,
        title, content
    );

    render_base(&title, &full_body, Some(wiki))
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
    let title = title.replace('-', " ").replace('_', " ");

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
