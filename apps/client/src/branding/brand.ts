// The product identity. Components read names, colours and links from here and never hardcode
// them, so renaming the product is a configuration change (branding/brand.json or APP_* variables).

export const BRAND = __BRAND__;
export const APP_VERSION = __APP_VERSION__;
export const BUILD_ID = __BUILD_ID__;
/** Brand assets (branding/assets) are served from the site root. */
export const LOGO_MARK_URL = '/logo-mark.svg';
