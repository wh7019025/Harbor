import { defineClientConfig } from "vuepress/client";
import "./theme/styles/custom.css";
import HarborLanding from "./components/HarborLanding.vue";

export default defineClientConfig({
  enhance({ app }) {
    app.component("HarborLanding", HarborLanding);
  },
});
