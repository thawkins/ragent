# Web source

- URL: https://github.com/tejgokani/CodeShift
- Title: GitHub - tejgokani/CodeShift: CodeShift is an automated code migration engine that rewrites your project from one...
- Author(s): -
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:33:32.992895344+00:00
- Relevance: Medium - multiple title terms match query


```text
CodeShift (`codeshift`, MIT) is an AST-driven refactor engine distributed via npm (`npm install -g codeshift` or `npx codeshift`) that applies conservative, plugin-style transforms to globbed files; it defaults to dry-run, requires `--apply` to write, creates `.codeshift.bak` backups by default (disable with `--no-backup`), and validates transformed syntax before writing. Built-in transforms include `express-to-fastify` (simple Express `router.get('/path', handler)` to `fastify.route({ method: 'GET', url: '/path', handler })`), CommonJS-to-ESM conversion (pure `require`/`module.exports` only; skips existing imports, dynamic `require()`, and `module.exports.foo` patterns), and a limited Mongoose-to-Prisma-like transform for simple `.find()`/`.findOne()` calls (not `.save()`, `.create()`, or complex queries; assumes Prisma client exists; lowercases model names). It reports summary counts and per-change details, warns that complex migrations require manual intervention, and advises reviewing changes before production.
```
