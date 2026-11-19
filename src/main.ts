import { createApp } from "vue";
import App from "./App.vue";
import { vTip } from "./tip";
import "./styles.css";

document.addEventListener("contextmenu", (event) => event.preventDefault());

createApp(App).directive("tip", vTip).mount("#app");