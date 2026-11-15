# Static website

`docs/` contains the single-page source. `npm ci && npm run build` uses the production Tailwind CLI to emit `target/site/style.css` and copies only deployable assets into `target/site/`. Neither `node_modules` nor output belongs in Git. Node 24 and locked npm dependencies are used in CI.

The existing static stack is retained. Native HTML `details`/`summary`, links, and `select` provide accessible disclosure, keyboard navigation, and theme controls without a JavaScript component runtime. React/Radix would add a second stack; a lightweight interaction library adds no benefit to these controls, so no extra component library is shipped. Layout/components use compiled Tailwind and the project’s CSS design tokens.

All local URLs are relative and work under `/ClipVault/` or a custom domain. Serve **the build output**, not raw `docs/` (which has CSS source). Example local preview:

```sh
npm ci
npm run build
python3 -m http.server 8080 --directory target/site
```

`theme.js` runs synchronously before the stylesheet. It reads the saved Light/Dark/System preference with a storage-denial fallback. System colors use a CSS media query, follow live system changes, and work before JavaScript finishes. Focus rings, skip navigation, native FAQ keyboard toggles, mobile layouts, and reduced-motion overrides are included.

`site.js` queries GitHub’s latest stable release endpoint and uses `release-data.js` to require exact versioned x86-64 names and repository URLs. It displays actual asset sizes, disables absent artifacts, excludes drafts/prereleases, and leaves a useful GitHub Releases link on errors or without JavaScript. Valid data is cached locally for at most 24 hours and revalidated on each visit. No source version is presented as the published version and no OS detection chooses for the visitor. The API call is made by the visitor’s browser and contacts GitHub. No PPA commands are advertised.

`history.png` is a real native app capture from the existing project, not a generated UI. Its exact provenance is in `docs/SCREENSHOT.md`. The small search graphic is explicitly labelled an illustration. No real clipboard contents are uploaded by the website.

The sole Pages workflow remains `.github/workflows/pages.yml`: builds/tests run with read-only repository permission, only deployment has Pages write/OIDC. Set **Settings → Pages → Source → GitHub Actions** once. Default-branch source changes, manual dispatch, and successful **Release packages** completions trigger it. The `workflow_run` trigger handles releases created with GITHUB_TOKEN, which do not trigger another release-event workflow; it checks out only the trusted default branch and never consumes workflow-run code/artifacts. Release data refreshes at runtime, so the website also updates without committing generated version files.

`npm test` checks static build assets, anchors, and release eligibility. `npm run test:browser` uses installed Chrome (override `CHROME_PATH`) and in-process local asset routing mounted at `/ClipVault/` (no listening server); it tests 1440, 768, 390, and 320px, theme persistence/system preference, FAQ keyboard controls, skip focus, offline fallbacks, image loading, and missing packages. Screenshots and JSON results are saved under `target/website-checks/` and uploaded by the Pages workflow. GitHub responses in browser tests are fixtures, not live release verification.
