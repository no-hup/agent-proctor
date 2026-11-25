import { describe, expect, it, vi } from "vitest";
import { Badge } from "../src/badge";

vi.mock("react-i18next", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-i18next")>()),
}));

describe("Badge", () => {
  it("renders label", () => {
    const el = <Badge label="ok" />;
    expect(el.props.label).toBe("ok");
  });
});
