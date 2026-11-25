/// <reference types="vite/client" />

/** Product identity resolved at build time from branding/brand.json and APP_* overrides. */
declare const __BRAND__: {
  name: string;
  shortName: string;
  description: string;
  tagline: string;
  primaryColor: string;
  secondaryColor: string;
  supportEmail: string;
  websiteUrl: string;
  termsUrl: string;
  privacyUrl: string;
  copyright: string;
  desktopIdentifier: string;
};
declare const __APP_VERSION__: string;
declare const __BUILD_ID__: string;

interface ImportMetaEnv {
  /** API origin for desktop builds (the web app always uses its own origin). */
  readonly VITE_API_BASE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
