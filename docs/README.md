# Project site

Static HTML/CSS concept site moved from the archived `dungeonflux.old/website/`.
Its 21 WebP assets are preserved. The original HTML/CSS/JavaScript and source
hashes are retained in the task's `work/site-migration/` evidence. The archived
local dice demo has been replaced with a fixed illustrated example and native
HTML disclosure; this site contains no authored JavaScript.

The new Rust game is in planning. Copy and calls to action distinguish concept
art and intended behavior from a playable release. This is a static project page,
not the game client. `CNAME` selects `dungeonfluxdnd.com`; `.nojekyll` keeps these
files static. Styles and media paths are relative so a repository subpath also
works; canonical/social URLs use the intended custom domain.

## Local preview

From the project root:

```sh
python3 -m http.server 8766 --bind 127.0.0.1 --directory docs
```

Open `http://127.0.0.1:8766/`. This is a development preview, not an application
runtime. Check section links, native FAQ/example disclosures, gallery scrolling,
asset loading and narrow layouts before publication.

## GitHub Pages setup when a repository is selected

1. Commit this folder to the chosen repository/branch. In Settings → Pages,
   select **Deploy from a branch**, that branch and **/docs**.
2. Verify ownership of the domain, set the Pages custom domain to
   `dungeonfluxdnd.com`, then configure the registrar's apex DNS to GitHub Pages
   using the current official instructions. A repository `CNAME` file alone
   does not configure GitHub or DNS.
3. After GitHub's domain checks and certificate succeed, enable HTTPS and inspect
   the published site, canonical metadata and every asset path. Keep the apex
   domain as the canonical URL; configure any optional `www` redirect deliberately.

See [publishing source](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site),
[custom domain setup](https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site)
and [static site entry/build behavior](https://docs.github.com/en/pages/getting-started-with-github-pages/creating-a-github-pages-site).
Official instructions checked 2026-09-29. No repository initialization, remote
Pages settings, DNS edits or publication were performed by this migration.
