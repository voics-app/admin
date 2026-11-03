import brand from "./brand.json";

/** The single source of the product name in the admin UI. Edit `src/brand.json`, nothing else. */
export const BRAND: { readonly name: string; readonly adminTitle: string } = brand;
