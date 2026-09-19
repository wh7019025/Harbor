/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<{}, {}, any>;
  export default component;
}

declare module "*.md?raw" {
  const content: string;
  export default content;
}

declare global {
  interface Window {
    __HARBOR_PANEL__?: {
      title: string;
      url: string;
    };
  }
}

export {};
