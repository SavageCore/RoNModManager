import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/svelte";
import { tick } from "svelte";
import CustomMenu, {
  type MenuItem,
} from "../../../src/lib/components/CustomMenu.svelte";
import CustomMenuProbe from "../fixtures/CustomMenuProbe.svelte";

const items: MenuItem[] = [
  { id: "edit", label: "Edit" },
  { id: "div1", divider: true },
  { id: "delete", label: "Delete", danger: true },
];

const CHECKED_ITEMS: MenuItem[] = [
  { id: "desktop", label: "Remove desktop shortcut", check: true },
  { id: "steam", label: "Add to Steam" },
];

type OpenOptions =
  { anchor?: HTMLElement; cursor?: { x: number; y: number } } | undefined;

async function openMenu(
  component: { openMenu?: (opts?: OpenOptions) => void },
  opts?: OpenOptions,
) {
  component.openMenu?.(opts ?? { cursor: { x: 100, y: 100 } });
  await tick();
  await tick();
}

describe("CustomMenu", () => {
  it("renders nothing while closed", () => {
    renderMenu();
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("renders items, dividers and danger styling when opened", async () => {
    const { component } = renderMenu();
    await openMenu(component);
    const menu = screen.getByRole("menu");
    expect(menu).not.toBeNull();
    expect(screen.getByText("Edit")).not.toBeNull();
    expect(screen.getByText("Delete")).not.toBeNull();
    expect(menu.querySelectorAll(".custom-menu-divider")).toHaveLength(1);
    expect(
      screen
        .getByText("Delete")
        .closest("button")
        ?.classList.contains("danger"),
    ).toBe(true);
  });

  it("shows a check mark for checked items", async () => {
    const { component } = renderMenu({ items: CHECKED_ITEMS });
    await openMenu(component);
    const row = screen.getByText("Remove desktop shortcut").closest("button");
    expect(row?.querySelector(".custom-menu-item-check")).not.toBeNull();
    expect(
      screen
        .getByText("Add to Steam")
        .closest("button")
        ?.querySelector(".custom-menu-item-check"),
    ).toBeNull();
  });

  it("runs the item action, closes and notifies the parent on click", async () => {
    const action = vi.fn();
    const { component, container } = render(CustomMenuProbe, {
      props: { items: [{ id: "go", label: "Go", action }] },
    });
    component.open();
    await tick();
    await fireEvent.click(screen.getByText("Go"));
    await tick();
    expect(action).toHaveBeenCalledTimes(1);
    expect(screen.getByText("closed-count: 1")).not.toBeNull();
    expect(container.querySelector(".custom-menu")).toBeNull();
  });

  it("does not run disabled item actions", async () => {
    const action = vi.fn();
    const { component } = render(CustomMenu, {
      props: { items: [{ id: "busy", label: "Busy", disabled: true, action }] },
    });
    await openMenu(component);
    await fireEvent.click(screen.getByText("Busy"));
    expect(action).not.toHaveBeenCalled();
  });

  it("closes and notifies the parent on Escape", async () => {
    const { component, container } = render(CustomMenuProbe, {
      props: { items },
    });
    component.open();
    await tick();
    await fireEvent.keyDown(document, { key: "Escape" });
    await tick();
    expect(screen.getByText("closed-count: 1")).not.toBeNull();
    expect(container.querySelector(".custom-menu")).toBeNull();
  });

  it("closes and notifies the parent on outside mousedown", async () => {
    const { component, container } = render(CustomMenuProbe, {
      props: { items },
    });
    component.open();
    await tick();
    await fireEvent.mouseDown(document.body);
    await tick();
    expect(screen.getByText("closed-count: 1")).not.toBeNull();
    expect(container.querySelector(".custom-menu")).toBeNull();
  });

  it("notifies the parent when closed programmatically", async () => {
    const { component } = render(CustomMenuProbe, { props: { items } });
    component.open();
    component.close();
    await tick();
    expect(screen.getByText("closed-count: 1")).not.toBeNull();
  });

  it("stays open when clicking inside the menu", async () => {
    const { component, container } = render(CustomMenu, { props: { items } });
    await openMenu(component);
    await fireEvent.mouseDown(screen.getByRole("menu"));
    expect(container.querySelector(".custom-menu")).not.toBeNull();
  });

  it("positions below-right of the cursor when opened with cursor", async () => {
    const { component, container } = renderMenu();
    await openMenu(component, { cursor: { x: 120, y: 80 } });
    const el = container.querySelector(".custom-menu") as HTMLElement;
    expect(el.style.left).toBe("126px");
    expect(el.style.top).toBe("86px");
  });

  it("positions against the anchor when opened with an anchor", async () => {
    const anchor = document.createElement("button");
    document.body.appendChild(anchor);
    const { component, container } = renderMenu();
    await openMenu(component, { anchor });
    const el = container.querySelector(".custom-menu") as HTMLElement;
    expect(el.style.left).not.toBe("auto");
    expect(el.style.top).not.toBe("auto");
    anchor.remove();
  });

  it("applies the width prop to the menu", async () => {
    const { component, container } = renderMenu({ width: 300 });
    await openMenu(component);
    const el = container.querySelector(".custom-menu") as HTMLElement;
    expect(el.style.width).toBe("300px");
  });

  it("shows the submenu title when opened with one", async () => {
    const { component, container } = renderMenu({
      items: [{ id: "edit", label: "Edit" }],
    });
    component.openMenu?.({ cursor: { x: 10, y: 10 }, title: "Fallout London" });
    await tick();
    await tick();
    expect(screen.getByText("Fallout London")).not.toBeNull();
    expect(container.querySelector(".custom-menu-title")).not.toBeNull();
  });

  it("reveals a nested submenu on parent hover", async () => {
    const action = vi.fn();
    const { component, container } = renderMenu({
      items: [
        { id: "refresh", label: "Refresh metadata" },
        {
          id: "tags",
          label: "Manage tags",
          children: [
            { id: "tag-a", label: "Tag A", check: true, action },
            { id: "tag-b", label: "Tag B", action },
          ],
        },
      ],
    });
    await openMenu(component);
    const parentRow = screen.getByText("Manage tags").closest("button");
    if (!parentRow) throw new Error("parent row missing");
    await fireEvent.mouseEnter(parentRow);
    await tick();
    await tick();
    const flyout = container.querySelector(".custom-submenu");
    expect(flyout).not.toBeNull();
    expect(
      screen
        .getByText("Tag A")
        .closest("button")
        ?.querySelector(".custom-menu-item-check"),
    ).not.toBeNull();
  });

  it("does not run the parent action when it has children", async () => {
    const action = vi.fn();
    const { component } = renderMenu({
      items: [
        {
          id: "tags",
          label: "Manage tags",
          children: [{ id: "tag-a", label: "Tag A" }],
          action,
        },
      ],
    });
    await openMenu(component);
    await fireEvent.click(screen.getByText("Manage tags"));
    expect(action).not.toHaveBeenCalled();
  });

  it("runs a submenu child action and closes the whole menu", async () => {
    const action = vi.fn();
    const { component, container } = render(CustomMenuProbe, {
      props: {
        items: [
          {
            id: "tags",
            label: "Manage tags",
            children: [{ id: "tag-a", label: "Tag A", action }],
          },
        ],
      },
    });
    component.open();
    await tick();
    const parentRow = screen.getByText("Manage tags").closest("button");
    if (!parentRow) throw new Error("parent row missing");
    await fireEvent.mouseEnter(parentRow);
    await tick();
    await tick();
    await fireEvent.click(screen.getByText("Tag A"));
    await tick();
    expect(action).toHaveBeenCalledTimes(1);
    expect(screen.getByText("closed-count: 1")).not.toBeNull();
    expect(container.querySelector(".custom-menu")).toBeNull();
  });

  it("opens the first submenu with ArrowRight and retracts with Escape", async () => {
    const { component, container } = renderMenu({
      items: [
        { id: "refresh", label: "Refresh metadata" },
        {
          id: "tags",
          label: "Manage tags",
          children: [{ id: "tag-a", label: "Tag A" }],
        },
      ],
    });
    await openMenu(component);
    await fireEvent.keyDown(document, { key: "ArrowRight" });
    await tick();
    expect(container.querySelector(".custom-submenu")).not.toBeNull();
    await fireEvent.keyDown(document, { key: "Escape" });
    await tick();
    expect(container.querySelector(".custom-submenu")).toBeNull();
    expect(container.querySelector(".custom-menu")).not.toBeNull();
  });

  it("marks expanded parents for assistive tech", async () => {
    const { component } = renderMenu({
      items: [
        {
          id: "tags",
          label: "Manage tags",
          children: [{ id: "tag-a", label: "Tag A" }],
        },
      ],
    });
    await openMenu(component);
    const parentRow = screen.getByText("Manage tags").closest("button");
    if (!parentRow) throw new Error("parent row missing");
    await fireEvent.mouseEnter(parentRow);
    await tick();
    await tick();
    expect(
      screen
        .getByText("Manage tags")
        .closest("button")
        ?.getAttribute("aria-haspopup"),
    ).toBe("menu");
    expect(
      screen
        .getByText("Manage tags")
        .closest("button")
        ?.getAttribute("aria-expanded"),
    ).toBe("true");
  });
});

function renderMenu(opts?: {
  items?: MenuItem[];
  width?: number;
  title?: string;
}) {
  return render(CustomMenu, {
    props: {
      items: opts?.items ?? items,
      width: opts?.width,
      menuTitle: opts?.title ?? null,
    },
  });
}
