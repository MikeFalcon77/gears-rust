import type { GearUiPlugin } from "@gears/ui-sdk";
export const plugin: GearUiPlugin = {
  apiVersion: 1,
  id: "service-discovery",
  displayName: "Discovery",
  navigation: [{ pageId: "discovery", label: "Discovery", icon: "boxes" }],
  pages: [
    {
      id: "discovery",
      title: "Discovery",
      path: "/discovery",
      load: () => import("./page"),
    },
  ],
  widgets: [],
};
