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

  it("opens from the keyboard and closes on Tab", async () => {
    render(CustomSelectProbe, { props: { options: OPTIONS } });
    const trigger = screen.getByRole("button");

    await fireEvent.keyDown(trigger, { key: " " });
    await tick();
    expect(screen.getAllByRole("option")).toHaveLength(3);

    await fireEvent.keyDown(trigger, { key: "Tab" });
    await tick();
    expect(screen.queryAllByRole("option")).toHaveLength(0);
  });

  it("moves the focused option with the arrow keys and selects with Enter", async () => {
    render(CustomSelectProbe, { props: { options: OPTIONS } });
    const trigger = screen.getByRole("button");

    await fireEvent.keyDown(trigger, { key: "ArrowDown" });
    await tick();
    await fireEvent.keyDown(trigger, { key: "ArrowDown" });
    await fireEvent.keyDown(trigger, { key: "ArrowUp" });
    await fireEvent.keyDown(trigger, { key: "ArrowDown" });
    await fireEvent.keyDown(trigger, { key: "Enter" });
    await tick();

    expect(screen.getByText("selected: light")).not.toBeNull();
  });

  it("closes on Escape and returns focus to the trigger", async () => {
    render(CustomSelectProbe, { props: { options: OPTIONS } });
    const trigger = screen.getByRole("button");

    await fireEvent.keyDown(trigger, { key: "ArrowDown" });
    await tick();
    await fireEvent.keyDown(trigger, { key: "Escape" });
    await tick();

    expect(screen.queryAllByRole("option")).toHaveLength(0);
    expect(document.activeElement).toBe(trigger);
  });

  it("ignores a disabled option chosen with the keyboard", async () => {
    const options = [
      { value: "system", label: "System" },
      { value: "dark", label: "Dark", disabled: true },
    ];
    render(CustomSelectProbe, { props: { options } });
    const trigger = screen.getByRole("button");

    await fireEvent.keyDown(trigger, { key: "ArrowDown" });
    await tick();
    await fireEvent.keyDown(trigger, { key: "ArrowDown" });
    await fireEvent.keyDown(trigger, { key: "Enter" });
    await tick();

    expect(screen.getByText("selected:")).not.toBeNull();
    expect(screen.getAllByRole("option")).toHaveLength(2);
  });

  it("closes when clicking outside the dropdown", async () => {
    render(CustomSelectProbe, { props: { options: OPTIONS } });

    await open();
    await fireEvent.mouseDown(document.body);
    await tick();

    expect(screen.queryAllByRole("option")).toHaveLength(0);
  });
});
