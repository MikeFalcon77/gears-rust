import type { GearUiPlugin } from "@gears/ui-sdk";
export const plugin: GearUiPlugin = {
  apiVersion: 1,
  id: "settings-service",
  displayName: "Settings",
  navigation: [{ pageId: "settings", label: "Settings", icon: "settings" }],
  pages: [
    {
      id: "settings",
      title: "Settings",
      path: "/settings",
      load: () => import("./settings-page"),
    },
  ],
  widgets: [],
};
