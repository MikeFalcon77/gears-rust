import type { GearUiPlugin } from "@gears/ui-sdk";
export const plugin: GearUiPlugin = {
  apiVersion: 1,
  id: "event-broker",
  displayName: "Event Broker",
  navigation: [{ pageId: "events", label: "Event Broker", icon: "activity" }],
  pages: [
    {
      id: "events",
      title: "Event Broker",
      path: "/events",
      load: () => import("./page"),
    },
  ],
  widgets: [],
};
