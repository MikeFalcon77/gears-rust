import type { GearUiPlugin } from "@gears/ui-sdk";
export const plugin: GearUiPlugin = {
  apiVersion: 1,
  id: "types-registry",
  displayName: "Types Registry V1",
  navigation: [{ pageId: "types", label: "Types Registry V1", icon: "boxes" }],
  pages: [
    {
      id: "types",
      title: "Types Registry V1",
      path: "/types",
      load: () => import("./page"),
    },
  ],
  widgets: [],
};
