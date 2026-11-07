import { useEffect } from "react";

/** Title, description and path of each page, for the document head and social cards. */
export const PAGE_META = {
  home: {
    path: "/",
    title: "Clave: private k-of-n multisig for Aztec",
    description:
      "Clave is a k-of-n multisig for Aztec. Payments need k member signatures, checked inside the proof, and the vault's notes can only be read when t members' keepers decrypt together.",
  },
  spending: {
    path: "/spending",
    title: "Spending: k-of-n signatures in an Aztec account contract | Clave",
    description:
      "How a Clave vault authorizes a payment: members sign one approval message, and the vault's account contract verifies k signatures inside the client-side proof.",
  },
  viewing: {
    path: "/viewing",
    title: "Viewing: threshold decryption of Aztec notes | Clave",
    description:
      "How a Clave vault reads its private notes: the viewing key is split across members' keepers, and any t of them decrypt together without the key ever existing.",
  },
  membership: {
    path: "/membership",
    title: "Membership: remove a member, keep the address | Clave",
    description:
      "How Clave changes members: the remaining keepers reshare the viewing key, so a removed member cannot read new notes and the vault keeps its Aztec address.",
  },
} as const;

const SITE = (import.meta.env.VITE_SITE_URL ?? "").replace(/\/$/, "");

function setMeta(selector: string, attribute: "name" | "property", key: string, value: string) {
  let el = document.head.querySelector<HTMLMetaElement>(selector);
  if (!el) {
    el = document.createElement("meta");
    el.setAttribute(attribute, key);
    document.head.append(el);
  }
  el.content = value;
}

/** Sets the page's title, description, canonical URL and social card text while it is mounted. */
export function usePageMeta(page: keyof typeof PAGE_META) {
  const { path, title, description } = PAGE_META[page];
  useEffect(() => {
    const url = `${SITE || window.location.origin}${path}`;
    document.title = title;
    setMeta('meta[name="description"]', "name", "description", description);
    setMeta('meta[property="og:title"]', "property", "og:title", title);
    setMeta('meta[property="og:description"]', "property", "og:description", description);
    setMeta('meta[property="og:url"]', "property", "og:url", url);
    setMeta('meta[name="twitter:title"]', "name", "twitter:title", title);
    setMeta('meta[name="twitter:description"]', "name", "twitter:description", description);
    document.head
      .querySelector<HTMLLinkElement>('link[rel="canonical"]')
      ?.setAttribute("href", url);
  }, [path, title, description]);
}
