import { describe, expect, it, vi } from "vitest";
import { Badge } from "../src/badge";

vi.mock("react-i18next", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-i18next")>()),
}));

describe("Badge", () => {
  it("renders label", () => {
    const el = <Badge label="updated" />;
    expect(el.props.label).toBe("updated");
  });
});
