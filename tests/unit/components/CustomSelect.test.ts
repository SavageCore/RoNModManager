import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/svelte";
import { tick } from "svelte";
import CustomSelectProbe from "../fixtures/CustomSelectProbe.svelte";

const OPTIONS = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

async function open() {
  await fireEvent.click(screen.getByRole("button"));
  await tick();
}

describe("CustomSelect", () => {
  it("shows the bound label while closed", () => {
    render(CustomSelectProbe, {
      props: { options: OPTIONS, initial: "light" },
    });
    expect(screen.getByRole("button").textContent).toContain("Light");
  });

  it("updates the bound value and emits select on choice", async () => {
    render(CustomSelectProbe, { props: { options: OPTIONS } });

    await open();
    await fireEvent.click(screen.getByText("Light"));
    await tick();

    expect(screen.getByText("value: light")).not.toBeNull();
    expect(screen.getByText("selected: light")).not.toBeNull();
  });

  it("does not emit select when the menu is merely opened", async () => {
    render(CustomSelectProbe, { props: { options: OPTIONS } });

    await open();

    expect(screen.getByText("selected:")).not.toBeNull();
  });
});
