import { resolve } from "node:path";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { type Plugin, defineConfig } from "vite";

/** Pages the sitemap lists; keep in step with src/router.tsx. */
const PAGES = ["/", "/spending", "/viewing", "/membership"];

/**
 * Absolute URLs for canonical links, Open Graph and Twitter cards come from VITE_SITE_URL (for example
 * https://clave.example). Without it, `%SITE_URL%` becomes empty: links stay relative, and no sitemap is written,
 * because a sitemap needs absolute URLs.
 */
function seo(): Plugin {
  const site = (process.env.VITE_SITE_URL ?? "").replace(/\/$/, "");
  return {
    name: "clave-seo",
    transformIndexHtml: (html) => html.replaceAll("%SITE_URL%", site),
    generateBundle() {
      const robots = [
        "User-agent: *",
        "Allow: /",
        ...(site ? [`Sitemap: ${site}/sitemap.xml`] : []),
      ];
      this.emitFile({ type: "asset", fileName: "robots.txt", source: `${robots.join("\n")}\n` });
      if (!site) {
        this.warn(
          "VITE_SITE_URL is not set: Open Graph and Twitter images use relative URLs; no sitemap.",
        );
        return;
      }
      const urls = PAGES.map((p) => `  <url><loc>${site}${p}</loc></url>`).join("\n");
      this.emitFile({
        type: "asset",
        fileName: "sitemap.xml",
        source: `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`,
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), tailwindcss(), seo()],
  resolve: { alias: { "@": resolve(import.meta.dirname, "src") } },
  server: { port: 3000, strictPort: true },
  preview: { port: 3000, strictPort: true },
  build: { outDir: "dist", sourcemap: false },
});
