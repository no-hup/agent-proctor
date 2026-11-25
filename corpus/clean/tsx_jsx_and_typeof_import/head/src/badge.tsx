import React from "react";
export function Badge({ label }: { label: string }) {
  return <div data-testid="badge">{label}</div>;
}
