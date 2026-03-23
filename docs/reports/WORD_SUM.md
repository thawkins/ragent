# Summary of testword1.docx

## Overview
This document describes Cloudflare's Browser Rendering API's new website crawling endpoint, currently available in open beta.

## Key Functionality
The crawling endpoint allows users to:
- Submit a single URL to crawl an entire website
- Automatically discover, render, and return pages in multiple formats (HTML, Markdown, and structured JSON)
- Run crawl jobs asynchronously using a job ID tracking system

## Intended Use Cases
- Training machine learning models
- Building RAG (Retrieval-Augmented Generation) pipelines
- Researching or monitoring website content

## How It Works
1. Submit a URL to initiate a crawl job
2. Receive a job ID in response
3. Poll the API with the job ID to check results as pages are processed

## Key Features

1. **Multiple Output Formats**: HTML, Markdown, and structured JSON (powered by Workers AI)
2. **Crawl Scope Controls**: Configure crawl depth, page limits, and wildcard patterns for URL inclusion/exclusion
3. **Automatic Page Discovery**: Discovers URLs from sitemaps, page links, or both
4. **Incremental Crawling**: Support for `modifiedSince` and `maxAge` parameters to skip unchanged pages and reduce costs
5. **Static Mode**: Option to fetch static HTML without browser rendering for faster crawling
6. **Bot Compliance**: Honors robots.txt directives including crawl-delay settings

## Availability
- Available on both Workers Free and Paid plans

## Getting Started
- Refer to the crawl endpoint documentation
- Review robots.txt and sitemaps best practices for site setup

## Example API Calls
The document includes example curl commands for:
- Initiating a crawl
- Checking crawl results using a job ID
