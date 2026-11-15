describe("Auth", () => {
  it("authenticates token", () => {
    expect(verifyToken("abc")).toBe(true);
  });
});
