import { reactive } from "vue";

export const tip = reactive({
  visible: false,
  text: "",
  x: 0,
  y: 0,
});

let timer: number | undefined;

function hide() {
  window.clearTimeout(timer);
  tip.visible = false;
}

type TipElement = HTMLElement & { __tipCleanup?: () => void };

export const vTip = {
  mounted(element: TipElement, binding: { value?: string }) {
    const show = (event: MouseEvent) => {
      const text = binding.value ?? "";
      if (!text) return;
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        tip.text = text;
        tip.x = event.clientX + 12;
        tip.y = event.clientY + 16;
        tip.visible = true;
      }, 400);
    };
    const move = (event: MouseEvent) => {
      if (!tip.visible) return;
      tip.x = event.clientX + 12;
      tip.y = event.clientY + 16;
    };
    element.addEventListener("mouseenter", show);
    element.addEventListener("mousemove", move);
    element.addEventListener("mouseleave", hide);
    element.addEventListener("mousedown", hide);
    element.__tipCleanup = () => {
      element.removeEventListener("mouseenter", show);
      element.removeEventListener("mousemove", move);
      element.removeEventListener("mouseleave", hide);
      element.removeEventListener("mousedown", hide);
    };
  },
  unmounted(element: TipElement) {
    element.__tipCleanup?.();
    hide();
  },
};