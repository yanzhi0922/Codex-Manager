export const ROOT_PAGE_PATHS = [
  "/",
  "/cockpit",
  "/accounts",
  "/aggregate-api",
  "/apikeys",
  "/sessions",
  "/logs",
  "/settings",
] as const;

export type RootPagePath = (typeof ROOT_PAGE_PATHS)[number];
