# Summary of testword1.docx

## Overview
This document describes Cloudflare's new Browser Rendering crawl endpoint, which is now available in open beta.

## Main Features
The crawl endpoint allows users to:
- Submit a starting URL and automatically discover, render, and return multiple pages
- Crawl entire websites with a single API call
- Retrieve results in multiple formats: HTML, Markdown, and structured JSON

## How It Works
1. Submit a URL to initiate a crawl job
2. Receive a job ID in response
3. Check back on the job as pages are processed asynchronously
4. Retrieve crawled content in your desired format

## Key Capabilities
- **Multiple Output Formats**: HTML, Markdown, and structured JSON powered by Workers AI
- **Crawl Scope Controls**: Configure depth, page limits, and URL patterns for inclusion/exclusion
- **Automatic Page Discovery**: Discovers URLs from sitemaps and page links
- **Incremental Crawling**: Supports `modifiedSince` and `maxAge` parameters to skip unchanged pages
- **Static Mode**: Option to fetch static HTML without browser rendering for faster crawling
- **Bot Etiquette**: Honors robots.txt directives including crawl-delay

## Use Cases
- Training machine learning models
- Building RAG (Retrieval-Augmented Generation) pipelines
- Researching and monitoring website content

## Availability
- Available on both Workers Free and Paid plans
- Documentation available in the crawl endpoint documentation
- Includes best practices for robots.txt and sitemaps

## API Example
The document includes curl examples showing how to:
- Initiate a crawl with a POST request
- Check results with a GET request using the job ID
